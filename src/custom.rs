use crate::{NbtValue, TextComponent, content::Content};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// The id and payload identifying a piece of custom content.
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
/// The NBT a [`CustomData`] carries, if any.
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Payload {
    #[default]
    Empty,
    Nbt(NbtValue),
}
impl Payload {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self == &Payload::Empty
    }
}

/// Maps ids to the content that renders them.
pub trait CustomRegistry {
    /// What the registry hands to [`CustomContent::resolve`].
    type Data;
    /// Registers content under an id.
    fn register_content<T: CustomContent>(&mut self, id: &'static str, content: T);
    /// The content registered under an id.
    fn get_content(&self, id: String) -> Box<dyn CustomContent<Reg = Self>>;
}

/// Content this crate renders by calling back into a registry.
pub trait CustomContent {
    /// The registry this content is registered in.
    type Reg: CustomRegistry;
    /// The id and payload that carry this content on the wire.
    fn as_data(&self) -> CustomData;
    /// Renders this content into a [`TextComponent`].
    fn resolve(&self, data: <Self::Reg as CustomRegistry>::Data, payload: Payload)
    -> TextComponent;
}

impl From<CustomData> for TextComponent {
    fn from(value: CustomData) -> Self {
        TextComponent {
            content: Content::Custom(value),
            ..Default::default()
        }
    }
}
impl<T: CustomContent> From<T> for TextComponent {
    fn from(value: T) -> Self {
        TextComponent::custom(value)
    }
}
