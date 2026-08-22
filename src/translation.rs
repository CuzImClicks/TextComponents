use crate::TextComponent;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::slice::Iter;

/// The `with` list of a [`TranslatedMessage`].
#[derive(Debug, Clone, Default, Eq)]
pub enum Args {
    #[default]
    None,
    Static(&'static [TextComponent]),
    Owned(Box<[TextComponent]>),
}

impl Args {
    /// Whether there is no `with` field at all.
    #[inline]
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Args::None)
    }

    /// The arguments, empty for [`Args::None`].
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[TextComponent] {
        match self {
            Args::None => &[],
            Args::Static(args) => args,
            Args::Owned(args) => args,
        }
    }

    /// Whether there are no arguments.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    /// Iterates the arguments.
    #[inline]
    pub fn iter(&self) -> Iter<'_, TextComponent> {
        self.as_slice().iter()
    }
}

impl<'a> IntoIterator for &'a Args {
    type Item = &'a TextComponent;
    type IntoIter = Iter<'a, TextComponent>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A list always converts to a present `with`, even when empty.
impl From<Vec<TextComponent>> for Args {
    fn from(args: Vec<TextComponent>) -> Self {
        Args::Owned(args.into_boxed_slice())
    }
}

impl From<Box<[TextComponent]>> for Args {
    fn from(args: Box<[TextComponent]>) -> Self {
        Args::Owned(args)
    }
}

impl<const N: usize> From<[TextComponent; N]> for Args {
    fn from(args: [TextComponent; N]) -> Self {
        Args::Owned(Box::new(args))
    }
}

impl From<&'static [TextComponent]> for Args {
    fn from(args: &'static [TextComponent]) -> Self {
        Args::Static(args)
    }
}

impl PartialEq for Args {
    fn eq(&self, other: &Self) -> bool {
        self.is_none() == other.is_none() && self.as_slice() == other.as_slice()
    }
}

impl Hash for Args {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.is_none().hash(state);
        self.as_slice().hash(state);
    }
}

#[cfg(feature = "serde")]
impl ::serde::Serialize for Args {
    fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Args::None => serializer.serialize_none(),
            _ => serializer.serialize_some(self.as_slice()),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> ::serde::Deserialize<'de> for Args {
    fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(
            match Option::<Box<[TextComponent]>>::deserialize(deserializer)? {
                Some(args) => Args::Owned(args),
                None => Args::None,
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A translation key with its arguments and fallback.
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct TranslatedMessage {
    #[cfg_attr(feature = "serde", serde(rename = "translate"))]
    pub key: Cow<'static, str>,
    /// Text shown when the key is unknown.
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub fallback: Option<Cow<'static, str>>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Args::is_none", rename = "with", default)
    )]
    pub args: Args,
}
impl TranslatedMessage {
    /// Creates a new `TranslatedMessage` without fallback.
    #[must_use]
    pub const fn new(key: &'static str, args: Args) -> Self {
        Self {
            key: Cow::Borrowed(key),
            args,
            fallback: None,
        }
    }

    /// Creates a new `TranslatedMessage` showing `fallback` when the key is unknown.
    #[must_use]
    pub const fn with_fallback(key: &'static str, fallback: &'static str, args: Args) -> Self {
        Self {
            key: Cow::Borrowed(key),
            args,
            fallback: Some(Cow::Borrowed(fallback)),
        }
    }

    /// The message as a [`TextComponent`].
    #[inline]
    #[must_use]
    pub const fn component(self) -> TextComponent {
        TextComponent::translated(self)
    }
    /// The message as a [`TextComponent`], showing `fallback` when the key is unknown.
    #[inline]
    pub fn component_fallback(mut self, fallback: impl Into<Cow<'static, str>>) -> TextComponent {
        self.fallback = Some(fallback.into());
        TextComponent::translated(self)
    }
}

impl From<TranslatedMessage> for TextComponent {
    fn from(value: TranslatedMessage) -> Self {
        value.component()
    }
}

/// A translation key that takes exactly `ARGS` arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Translation<const ARGS: usize>(pub &'static str);

impl Translation<0> {
    /// Creates a new `TranslatedMessage` with no arguments.
    #[must_use]
    pub const fn msg(&self) -> TranslatedMessage {
        TranslatedMessage::new(self.0, Args::None)
    }

    /// The message pre-encoded as network NBT, ready to write into a packet.
    #[cfg(feature = "nbt")]
    #[must_use]
    pub fn encode(&self) -> crate::EncodedComponent {
        self.msg().encode()
    }
}

impl<const ARGS: usize> Translation<ARGS> {
    /// Creates a new `TranslatedMessage` with the given arguments.
    #[must_use]
    pub fn message(&self, args: [impl Into<TextComponent>; ARGS]) -> TranslatedMessage {
        TranslatedMessage::new(self.0, Args::Owned(Box::new(args.map(Into::into))))
    }

    /// The message pre-encoded as network NBT, ready to write into a packet.
    #[cfg(feature = "nbt")]
    #[must_use]
    pub fn encode_with(&self, args: [impl Into<TextComponent>; ARGS]) -> crate::EncodedComponent {
        self.message(args).encode()
    }
}

impl From<&Translation<0>> for TextComponent {
    fn from(value: &Translation<0>) -> Self {
        value.msg().component()
    }
}
