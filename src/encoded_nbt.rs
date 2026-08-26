use simdnbt::owned::NbtTag;
#[cfg(feature = "nbt")]
use simdnbt::owned::read_tag;
use std::borrow::Cow;
#[cfg(feature = "nbt")]
use std::error::Error;
#[cfg(feature = "nbt")]
use std::fmt::{self, Display, Formatter};
#[cfg(feature = "nbt")]
use std::io::Cursor;

use crate::NbtValue;
#[cfg(feature = "nbt")]
use crate::translation::{TranslatedMessage, Translation};
#[cfg(feature = "nbt")]
use crate::{TextComponent, nbt::DecodeError, resolving::TextResolutor};

/// A text component pre-encoded as network NBT (`[tag type][payload]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedComponent(Cow<'static, [u8]>);

impl EncodedComponent {
    /// Wraps already-encoded `'static` bytes.
    #[must_use]
    pub const fn from_static(bytes: &'static [u8]) -> Self {
        Self(Cow::Borrowed(bytes))
    }

    /// Wraps already-encoded bytes.
    #[must_use]
    pub const fn from_vec(bytes: Vec<u8>) -> Self {
        Self(Cow::Owned(bytes))
    }

    /// The encoded bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        match &self.0 {
            Cow::Borrowed(bytes) => bytes,
            Cow::Owned(bytes) => bytes.as_slice(),
        }
    }

    /// Takes the encoded bytes.
    #[must_use]
    pub fn into_bytes(self) -> Cow<'static, [u8]> {
        self.0
    }

    /// The encoded bytes read back as an NBT tag.
    ///
    /// # Panics
    /// If the bytes are not valid NBT.
    #[cfg(feature = "nbt")]
    #[must_use]
    pub fn to_nbt_tag(&self) -> NbtTag {
        read_tag(&mut Cursor::new(self.as_bytes())).expect("an encoded component is valid NBT")
    }

    /// Decodes the bytes back into a [`TextComponent`].
    #[cfg(feature = "nbt")]
    pub fn decode(&self) -> Result<TextComponent, DecodeError> {
        let mut cursor = Cursor::new(self.as_bytes());
        let tag = read_tag(&mut cursor).map_err(simdnbt::Error::from)?;
        Ok(TextComponent::try_from_nbt(&tag)?)
    }

    /// The message as plain text.
    #[cfg(feature = "nbt")]
    pub fn to_plain<R: TextResolutor + ?Sized>(
        &self,
        resolutor: &R,
    ) -> Result<String, DecodeError> {
        Ok(self.decode()?.to_plain(resolutor))
    }
}

/// An opaque NBT payload encoded by another system's codec.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EncodedNbt(NbtValue);

/// Encodes a registry-owned value for embedding in a text component.
pub trait EmbeddedNbtCodec {
    type Error;

    /// Turns the value into the tag vanilla's codec writes for it; the component embeds it unchanged.
    fn encode_embedded_nbt(self) -> Result<NbtTag, Self::Error>;
}

impl EncodedNbt {
    /// Encodes a value through its [`EmbeddedNbtCodec`].
    pub fn encode<T: EmbeddedNbtCodec>(value: T) -> Result<Self, T::Error> {
        value.encode_embedded_nbt().map(Self::from_codec_output)
    }

    #[must_use]
    pub const fn as_nbt(&self) -> &NbtTag {
        self.0.as_nbt()
    }

    #[must_use]
    pub fn into_nbt(self) -> NbtTag {
        self.0.into_nbt()
    }

    /// Whether this is a compound with no entries.
    #[must_use]
    pub fn is_empty_compound(&self) -> bool {
        self.0.is_empty_compound()
    }

    pub(crate) const fn from_codec_output(value: NbtTag) -> Self {
        Self(NbtValue::new(value))
    }
}

/// Content a [`TextComponent`] carries that only [resolution](TextComponent::resolve) can fill in.
#[cfg(feature = "nbt")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnresolvedContent {
    /// A [`Resolvable::Scoreboard`](crate::content::Resolvable::Scoreboard) value.
    Scoreboard,
    /// A [`Resolvable::Entity`](crate::content::Resolvable::Entity) selector.
    Entity,
    /// A [`Resolvable::NBT`](crate::content::Resolvable::NBT) value.
    Nbt,
    /// A [`Content::Custom`](crate::content::Content::Custom) payload.
    #[cfg(feature = "custom")]
    Custom,
}

#[cfg(feature = "nbt")]
impl UnresolvedContent {
    const fn name(self) -> &'static str {
        match self {
            Self::Scoreboard => "a scoreboard value",
            Self::Entity => "an entity selector",
            Self::Nbt => "an NBT value",
            #[cfg(feature = "custom")]
            Self::Custom => "custom content",
        }
    }
}

#[cfg(feature = "nbt")]
impl Display for UnresolvedContent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the component contains {}, which has to be resolved before it can be encoded",
            self.name()
        )
    }
}

#[cfg(feature = "nbt")]
impl Error for UnresolvedContent {}

#[cfg(feature = "nbt")]
impl TextComponent {
    /// Encodes this component into the network NBT a packet writes.
    ///
    /// [`From`] does the same and panics instead; pick whichever suits the call site.
    ///
    /// # Errors
    /// If the component still holds content that needs [resolution](TextComponent::resolve).
    pub fn try_encode(&self) -> Result<EncodedComponent, UnresolvedContent> {
        if let Some(unresolved) = self.unresolved() {
            return Err(unresolved);
        }
        let mut buf = Vec::with_capacity(128);
        self.to_codec_nbt().write(&mut buf);
        Ok(EncodedComponent::from_vec(buf))
    }

    /// Encodes this component into the network NBT a packet writes.
    ///
    /// # Panics
    /// If the component still holds content that needs [resolution](TextComponent::resolve);
    /// use [`try_encode`](TextComponent::try_encode) to handle that case.
    #[must_use]
    #[track_caller]
    pub fn encode(&self) -> EncodedComponent {
        match self.try_encode() {
            Ok(encoded) => encoded,
            Err(unresolved) => panic!("{unresolved}"),
        }
    }

    /// The first content below this component that resolution has to fill in.
    #[must_use]
    pub fn unresolved(&self) -> Option<UnresolvedContent> {
        use crate::content::{Content, Object, Resolvable};
        use crate::interactivity::HoverEvent;

        match &self.content {
            Content::Resolvable(resolvable) => {
                return Some(match resolvable {
                    Resolvable::Scoreboard { .. } => UnresolvedContent::Scoreboard,
                    Resolvable::Entity { .. } => UnresolvedContent::Entity,
                    Resolvable::NBT { .. } => UnresolvedContent::Nbt,
                });
            }
            #[cfg(feature = "custom")]
            Content::Custom(_) => return Some(UnresolvedContent::Custom),
            Content::Translate(message) => {
                for arg in &message.args {
                    if let Some(unresolved) = arg.unresolved() {
                        return Some(unresolved);
                    }
                }
            }
            Content::Object(object) => {
                let fallback = match object {
                    Object::Atlas { fallback, .. } | Object::Player { fallback, .. } => fallback,
                };
                if let Some(fallback) = fallback
                    && let Some(unresolved) = fallback.get().unresolved()
                {
                    return Some(unresolved);
                }
            }
            Content::Text { .. } | Content::Keybind { .. } => {}
        }

        if let Some(hover) = &self.interactions.hover
            && let HoverEvent::ShowText { value } = hover.get()
            && let Some(unresolved) = value.get().unresolved()
        {
            return Some(unresolved);
        }

        self.children.iter().find_map(TextComponent::unresolved)
    }
}

/// # Panics
/// If the component holds content that needs [resolution](TextComponent::resolve);
/// use [`try_encode`](TextComponent::try_encode) to handle that case.
#[cfg(feature = "nbt")]
impl From<&TextComponent> for EncodedComponent {
    fn from(component: &TextComponent) -> Self {
        component.encode()
    }
}

/// # Panics
/// If the component holds content that needs [resolution](TextComponent::resolve);
/// use [`try_encode`](TextComponent::try_encode) to handle that case.
#[cfg(feature = "nbt")]
impl From<TextComponent> for EncodedComponent {
    fn from(component: TextComponent) -> Self {
        component.encode()
    }
}

#[cfg(feature = "nbt")]
impl From<&'static str> for EncodedComponent {
    fn from(text: &'static str) -> Self {
        TextComponent::const_plain(text).encode()
    }
}

#[cfg(feature = "nbt")]
impl From<String> for EncodedComponent {
    fn from(text: String) -> Self {
        TextComponent::plain(text).encode()
    }
}

/// # Panics
/// If an argument holds content that needs [resolution](TextComponent::resolve).
#[cfg(feature = "nbt")]
impl From<TranslatedMessage> for EncodedComponent {
    fn from(message: TranslatedMessage) -> Self {
        TextComponent::from(message).encode()
    }
}

#[cfg(feature = "nbt")]
impl From<&Translation<0>> for EncodedComponent {
    fn from(translation: &Translation<0>) -> Self {
        TextComponent::from(translation).encode()
    }
}
