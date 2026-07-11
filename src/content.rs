#[cfg(feature = "custom")]
use crate::custom::CustomData;
use crate::{
    TextComponent, format::Format, interactivity::Interactivity, translation::TranslatedMessage,
};
use std::borrow::Cow;

#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case", untagged))]
pub enum Content {
    Text {
        text: Cow<'static, str>,
    },
    Translate(TranslatedMessage),
    Keybind {
        keybind: Cow<'static, str>,
    },
    /// #### Needs [resolution](TextComponent::resolve)
    #[cfg(feature = "custom")]
    Custom(CustomData),
    Object(Object),
    /// #### Needs [resolution](TextComponent::resolve)
    Resolvable(Resolvable),
}

impl From<String> for Content {
    fn from(value: String) -> Self {
        Content::Text {
            text: Cow::Owned(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Object {
    Atlas {
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "is_default_atlas", default = "default_atlas")
        )]
        atlas: Cow<'static, str>,
        sprite: Cow<'static, str>,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Option::is_none", default)
        )]
        fallback: Option<Box<TextComponent>>,
    },
    Player {
        player: ObjectPlayer,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Clone::clone", default)
        )]
        hat: bool,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Option::is_none", default)
        )]
        fallback: Option<Box<TextComponent>>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct ObjectPlayer {
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub name: Option<Cow<'static, str>>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub id: Option<[i32; 4]>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub texture: Option<Cow<'static, str>>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub cape: Option<Cow<'static, str>>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub elytra: Option<Cow<'static, str>>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub model: Option<PlayerModel>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Vec::is_empty", default)
    )]
    pub properties: Vec<PlayerProperties>,
}
impl ObjectPlayer {
    /// Creates a [ObjectPlayer] from a player's name.
    pub fn name<T: Into<Cow<'static, str>>>(name: T) -> Self {
        ObjectPlayer {
            name: Some(name.into()),
            id: None,
            texture: None,
            cape: None,
            elytra: None,
            model: None,
            properties: vec![],
        }
    }
    /// Creates a [ObjectPlayer] from the id of a player.
    pub fn id(id: [i32; 4]) -> Self {
        ObjectPlayer {
            name: None,
            id: Some(id),
            texture: None,
            cape: None,
            elytra: None,
            model: None,
            properties: vec![],
        }
    }
    /// Creates a [ObjectPlayer] from the path to a texture of a resource pack.
    pub fn texture<T: Into<Cow<'static, str>>>(path: T) -> Self {
        ObjectPlayer {
            name: None,
            id: None,
            texture: Some(path.into()),
            cape: None,
            elytra: None,
            model: None,
            properties: vec![],
        }
    }
    /// Creates a [ObjectPlayer] from a player's skin properties.
    /// * `value` - A [texture data json](https://minecraft.wiki/w/Mojang_API#Query_player's_skin_and_cape) encoded in Base64
    /// * `signature` - An optional Mojang's signature, also encoded in Base64
    pub fn property<T: Into<Cow<'static, str>>, R: Into<Cow<'static, str>>>(
        value: T,
        signature: Option<R>,
    ) -> Self {
        ObjectPlayer {
            name: None,
            id: None,
            texture: None,
            cape: None,
            elytra: None,
            model: None,
            properties: vec![PlayerProperties {
                name: Cow::Borrowed("textures"),
                value: value.into(),
                signature: signature.map(Into::into),
            }],
        }
    }
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.id.is_none()
            && self.texture.is_none()
            && self.cape.is_none()
            && self.elytra.is_none()
            && self.model.is_none()
            && self.properties.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PlayerModel {
    Slim,
    Wide,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct PlayerProperties {
    pub name: Cow<'static, str>,
    pub value: Cow<'static, str>,
    pub signature: Option<Cow<'static, str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum Resolvable {
    /// The selector must only accept 1 target
    /// #### Needs [resolution](TextComponent::resolve)
    #[cfg_attr(feature = "serde", serde(rename = "score"))]
    Scoreboard {
        #[cfg_attr(feature = "serde", serde(rename = "name"))]
        selector: Cow<'static, str>,
        objective: Cow<'static, str>,
    },
    /// #### Needs [resolution](TextComponent::resolve)
    #[cfg_attr(feature = "serde", serde(untagged))]
    Entity {
        selector: Cow<'static, str>,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Option::is_none", default)
        )]
        separator: Option<Box<TextComponent>>,
    },
    /// #### Needs [resolution](TextComponent::resolve)
    #[cfg_attr(feature = "serde", serde(untagged))]
    NBT {
        #[cfg_attr(feature = "serde", serde(rename = "nbt"))]
        path: Cow<'static, str>,
        /// Whether selected NBT values should be decoded as components.
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false", default))]
        interpret: bool,
        /// Whether non-interpreted NBT should omit rich type styling.
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false", default))]
        plain: bool,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Option::is_none", default)
        )]
        separator: Option<Box<TextComponent>>,
        #[cfg_attr(feature = "serde", serde(flatten, default = "NbtSource::Entity"))]
        source: NbtSource,
    },
}

#[cfg(feature = "serde")]
const fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(feature = "serde")]
fn default_atlas() -> Cow<'static, str> {
    Cow::Borrowed("minecraft:blocks")
}

#[cfg(feature = "serde")]
fn is_default_atlas(value: &str) -> bool {
    value == "minecraft:blocks"
}
impl Resolvable {
    pub fn entity_separator() -> Box<TextComponent> {
        Box::new(TextComponent {
            content: Content::Text {
                text: Cow::Borrowed(", "),
            },
            format: Format {
                color: Some(crate::format::Color::Gray),
                ..Default::default()
            },
            ..Default::default()
        })
    }
    pub fn nbt_separator() -> Box<TextComponent> {
        Box::new(TextComponent {
            content: Content::Text {
                text: Cow::Borrowed(", "),
            },
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum NbtSource {
    Entity(Cow<'static, str>),
    Block(Cow<'static, str>),
    Storage(Cow<'static, str>),
}
impl NbtSource {
    /// Creates a [NbtSource] from a entity selector.
    pub fn entity<T: Into<Cow<'static, str>>>(selector: T) -> Self {
        NbtSource::Entity(selector.into())
    }
    /// Creates a [NbtSource] from a block coordinates.
    pub fn block(x: i32, y: i32, z: i32) -> Self {
        NbtSource::Block(Cow::Owned(format!("{x} {y} {z}")))
    }
    /// Creates a [NbtSource] from a Nbt Storage identifier.
    pub fn storage<T: Into<Cow<'static, str>>>(identifier: T) -> Self {
        NbtSource::Storage(identifier.into())
    }
}

impl From<Content> for TextComponent {
    fn from(value: Content) -> Self {
        TextComponent {
            content: value,
            children: Vec::new(),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}
impl From<Object> for TextComponent {
    fn from(value: Object) -> Self {
        TextComponent {
            content: Content::Object(value),
            children: Vec::new(),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}
impl From<ObjectPlayer> for TextComponent {
    fn from(value: ObjectPlayer) -> Self {
        TextComponent {
            content: Content::Object(Object::Player {
                player: value,
                hat: true,
                fallback: None,
            }),
            children: Vec::new(),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}
impl From<Resolvable> for TextComponent {
    fn from(value: Resolvable) -> Self {
        TextComponent {
            content: Content::Resolvable(value),
            children: Vec::new(),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}
