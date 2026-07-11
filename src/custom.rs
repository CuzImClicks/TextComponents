use crate::{NbtValue, TextComponent};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct CustomData {
    pub id: Cow<'static, str>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Payload::is_empty", default)
    )]
    pub payload: Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Payload {
    #[default]
    Empty,
    Nbt(NbtValue),
}
impl Payload {
    pub fn is_empty(&self) -> bool {
        self == &Payload::Empty
    }
}

pub trait CustomRegistry {
    type Data;
    fn register_content<T: CustomContent>(&mut self, id: &'static str, content: T);
    fn get_content(&self, id: String) -> Box<dyn CustomContent<Reg = Self>>;
}

pub trait CustomContent {
    type Reg: CustomRegistry;
    fn as_data(&self) -> CustomData;
    fn resolve(&self, data: <Self::Reg as CustomRegistry>::Data, payload: Payload)
    -> TextComponent;
}

impl From<CustomData> for TextComponent {
    fn from(value: CustomData) -> Self {
        TextComponent {
            content: crate::content::Content::Custom(value),
            ..Default::default()
        }
    }
}
impl<T: CustomContent> From<T> for TextComponent {
    fn from(value: T) -> Self {
        TextComponent::custom(value)
    }
}
