use std::{
    fmt::{self, Debug, Formatter},
    hash::{Hash, Hasher},
};

use simdnbt::{ToNbtTag, owned::NbtTag};

/// An arbitrary NBT value embedded in a text component.
#[derive(Clone)]
pub struct NbtValue(NbtTag);

impl NbtValue {
    #[must_use]
    pub(crate) const fn new(value: NbtTag) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_nbt(&self) -> &NbtTag {
        &self.0
    }

    #[must_use]
    pub fn into_nbt(self) -> NbtTag {
        self.0
    }

    #[must_use]
    pub(crate) fn is_empty_compound(&self) -> bool {
        matches!(&self.0, NbtTag::Compound(compound) if compound.is_empty())
    }

    fn encoded(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        self.0.write(&mut encoded);
        encoded
    }
}

impl Debug for NbtValue {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl PartialEq for NbtValue {
    fn eq(&self, other: &Self) -> bool {
        self.encoded() == other.encoded()
    }
}

impl Eq for NbtValue {}

impl Hash for NbtValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.encoded().hash(state);
    }
}

impl From<NbtTag> for NbtValue {
    fn from(value: NbtTag) -> Self {
        Self::new(value)
    }
}

impl From<NbtValue> for NbtTag {
    fn from(value: NbtValue) -> Self {
        value.into_nbt()
    }
}

impl ToNbtTag for NbtValue {
    fn to_nbt_tag(self) -> NbtTag {
        self.into_nbt()
    }
}

impl ToNbtTag for &NbtValue {
    fn to_nbt_tag(self) -> NbtTag {
        self.as_nbt().clone()
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    use std::fmt;

    use serde::{
        Deserialize, Deserializer, Serialize, Serializer,
        de::{Error, MapAccess, SeqAccess, Visitor},
    };
    use simdnbt::{
        Mutf8String,
        owned::{NbtCompound, NbtList, NbtTag},
    };

    use super::NbtValue;

    impl Serialize for NbtValue {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            self.as_nbt().serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for NbtValue {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_any(NbtValueVisitor)
        }
    }

    struct NbtValueVisitor;

    impl<'de> Visitor<'de> for NbtValueVisitor {
        type Value = NbtValue;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an NBT-compatible value")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
            Ok(NbtTag::Byte(i8::from(value)).into())
        }

        fn visit_i8<E>(self, value: i8) -> Result<Self::Value, E> {
            Ok(NbtTag::Byte(value).into())
        }

        fn visit_i16<E>(self, value: i16) -> Result<Self::Value, E> {
            Ok(NbtTag::Short(value).into())
        }

        fn visit_i32<E>(self, value: i32) -> Result<Self::Value, E> {
            Ok(NbtTag::Int(value).into())
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
            Ok(NbtTag::Long(value).into())
        }

        fn visit_u8<E>(self, value: u8) -> Result<Self::Value, E>
        where
            E: Error,
        {
            self.visit_i16(i16::from(value))
        }

        fn visit_u16<E>(self, value: u16) -> Result<Self::Value, E>
        where
            E: Error,
        {
            self.visit_i32(i32::from(value))
        }

        fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
        where
            E: Error,
        {
            self.visit_i64(i64::from(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            let value = i64::try_from(value).map_err(|_| E::custom("NBT integers are signed"))?;
            self.visit_i64(value)
        }

        fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E> {
            Ok(NbtTag::Float(value).into())
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
            Ok(NbtTag::Double(value).into())
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(NbtTag::String(value.into()).into())
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(NbtTag::String(value.into()).into())
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = sequence.next_element::<NbtValue>()? {
                values.push(value.into_nbt());
            }
            Ok(NbtTag::List(to_list(values).map_err(A::Error::custom)?).into())
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some((name, value)) = map.next_entry::<String, NbtValue>()? {
                values.push((Mutf8String::from(name), value.into_nbt()));
            }
            Ok(NbtTag::Compound(NbtCompound::from_values(values)).into())
        }
    }

    fn to_list(values: Vec<NbtTag>) -> Result<NbtList, &'static str> {
        let Some(first) = values.first() else {
            return Ok(NbtList::Empty);
        };
        macro_rules! collect {
            ($variant:ident, $pattern:pat => $value:expr) => {{
                let mut result = Vec::with_capacity(values.len());
                for tag in values {
                    let $pattern = tag else {
                        return Err("NBT lists must contain one tag type");
                    };
                    result.push($value);
                }
                Ok(NbtList::$variant(result))
            }};
        }

        match first {
            NbtTag::Byte(_) => collect!(Byte, NbtTag::Byte(value) => value),
            NbtTag::Short(_) => collect!(Short, NbtTag::Short(value) => value),
            NbtTag::Int(_) => collect!(Int, NbtTag::Int(value) => value),
            NbtTag::Long(_) => collect!(Long, NbtTag::Long(value) => value),
            NbtTag::Float(_) => collect!(Float, NbtTag::Float(value) => value),
            NbtTag::Double(_) => collect!(Double, NbtTag::Double(value) => value),
            NbtTag::ByteArray(_) => {
                collect!(ByteArray, NbtTag::ByteArray(value) => value)
            }
            NbtTag::String(_) => collect!(String, NbtTag::String(value) => value),
            NbtTag::List(_) => collect!(List, NbtTag::List(value) => value),
            NbtTag::Compound(_) => {
                collect!(Compound, NbtTag::Compound(value) => value)
            }
            NbtTag::IntArray(_) => collect!(IntArray, NbtTag::IntArray(value) => value),
            NbtTag::LongArray(_) => {
                collect!(LongArray, NbtTag::LongArray(value) => value)
            }
        }
    }
}
