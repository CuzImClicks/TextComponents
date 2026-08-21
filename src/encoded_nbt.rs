use simdnbt::owned::NbtTag;
#[cfg(feature = "nbt")]
use simdnbt::owned::read_tag;
use std::borrow::Cow;
#[cfg(feature = "nbt")]
use std::io::Cursor;

use crate::NbtValue;
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
