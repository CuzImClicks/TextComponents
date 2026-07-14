use simdnbt::owned::NbtTag;

use crate::NbtValue;

/// An NBT payload produced by a registry-owned codec.
///
/// Text components embed a few values whose schemas are owned by other
/// systems, such as item component patches and inline dialogs. The owning
/// system encodes those values before attaching them to a component, and the
/// component codec preserves the resulting tag verbatim.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct EncodedNbt(NbtValue);

/// Encodes a registry-owned value for embedding in a text component.
///
/// This trait is defined here so registry implementations can provide the
/// codec without making this crate depend on them.
pub trait EmbeddedNbtCodec {
    type Error;

    fn encode_embedded_nbt(self) -> Result<NbtTag, Self::Error>;
}

impl EncodedNbt {
    pub fn encode<T: EmbeddedNbtCodec>(value: T) -> Result<Self, T::Error> {
        value.encode_embedded_nbt().map(Self::from_codec_output)
    }

    pub const fn as_nbt(&self) -> &NbtTag {
        self.0.as_nbt()
    }

    pub fn into_nbt(self) -> NbtTag {
        self.0.into_nbt()
    }

    pub fn is_empty_compound(&self) -> bool {
        self.0.is_empty_compound()
    }

    pub(crate) const fn from_codec_output(value: NbtTag) -> Self {
        Self(NbtValue::new(value))
    }
}
