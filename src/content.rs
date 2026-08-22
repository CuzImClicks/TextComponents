#[cfg(feature = "custom")]
use crate::custom::CustomData;
use crate::{
    TextComponent,
    format::{Color, Format},
    interactivity::{Interactivity, MaybeStatic},
    translation::TranslatedMessage,
};
use std::borrow::Cow;

#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
/// What a component displays.
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
/// An image drawn inline with the text.
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
        fallback: Option<MaybeStatic<TextComponent>>,
    },
    Player {
        /// The player profile to render.
        player: MaybeStatic<ObjectPlayer>,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Clone::clone", default)
        )]
        hat: bool,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Option::is_none", default)
        )]
        fallback: Option<MaybeStatic<TextComponent>>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// The player a head object renders, named by any one of these fields.
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
        serde(skip_serializing_if = "has_no_properties", default)
    )]
    pub properties: Cow<'static, [PlayerProperties]>,
}
impl ObjectPlayer {
    /// Creates a [`ObjectPlayer`] from a player's name.
    pub fn name<T: Into<Cow<'static, str>>>(name: T) -> Self {
        ObjectPlayer {
            name: Some(name.into()),
            id: None,
            texture: None,
            cape: None,
            elytra: None,
            model: None,
            properties: Cow::Borrowed(&[]),
        }
    }
    // TODO: take impl Into<Cow<'static, str>> when From<&str> for Cow is const
    /// Creates a [`ObjectPlayer`] from a player's name known at compile time.
    #[must_use]
    pub const fn const_name(name: &'static str) -> Self {
        ObjectPlayer {
            name: Some(Cow::Borrowed(name)),
            id: None,
            texture: None,
            cape: None,
            elytra: None,
            model: None,
            properties: Cow::Borrowed(&[]),
        }
    }
    /// Creates a [`ObjectPlayer`] from the id of a player.
    #[must_use]
    pub const fn id(id: [i32; 4]) -> Self {
        ObjectPlayer {
            name: None,
            id: Some(id),
            texture: None,
            cape: None,
            elytra: None,
            model: None,
            properties: Cow::Borrowed(&[]),
        }
    }
    /// Creates a [`ObjectPlayer`] from the path to a texture of a resource pack.
    pub fn texture<T: Into<Cow<'static, str>>>(path: T) -> Self {
        ObjectPlayer {
            name: None,
            id: None,
            texture: Some(path.into()),
            cape: None,
            elytra: None,
            model: None,
            properties: Cow::Borrowed(&[]),
        }
    }
    /// Creates a [`ObjectPlayer`] from a player's skin properties.
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
            properties: Cow::Owned(vec![PlayerProperties {
                name: Cow::Borrowed("textures"),
                value: value.into(),
                signature: signature.map(Into::into),
            }]),
        }
    }
    #[must_use]
    pub(crate) const fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.id.is_none()
            && self.texture.is_none()
            && self.cape.is_none()
            && self.elytra.is_none()
            && self.model.is_none()
            && match &self.properties {
                Cow::Borrowed(properties) => properties.is_empty(),
                Cow::Owned(properties) => properties.is_empty(),
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
/// The arm width of a player skin.
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PlayerModel {
    Slim,
    Wide,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A signed skin property from the session server.
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct PlayerProperties {
    pub name: Cow<'static, str>,
    pub value: Cow<'static, str>,
    pub signature: Option<Cow<'static, str>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// Content whose value the server has to look up.
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum Resolvable {
    /// The selector must only accept 1 target
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
        separator: Option<MaybeStatic<TextComponent>>,
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
        separator: Option<MaybeStatic<TextComponent>>,
        #[cfg_attr(feature = "serde", serde(flatten, default = "NbtSource::Entity"))]
        source: NbtSource,
    },
}

#[cfg(feature = "serde")]
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde `skip_serializing_if` needs `fn(&T) -> bool`"
)]
const fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(feature = "serde")]
const fn default_atlas() -> Cow<'static, str> {
    Cow::Borrowed("minecraft:blocks")
}

#[cfg(feature = "serde")]
fn is_default_atlas(value: &str) -> bool {
    value == "minecraft:blocks"
}

#[cfg(feature = "serde")]
const fn has_no_properties(value: &[PlayerProperties]) -> bool {
    value.is_empty()
}

static ENTITY_SEPARATOR: TextComponent = TextComponent {
    content: Content::Text {
        text: Cow::Borrowed(", "),
    },
    children: Cow::Borrowed(&[]),
    format: Format::new().color(Color::Gray),
    interactions: Interactivity::new(),
};

static NBT_SEPARATOR: TextComponent = TextComponent {
    content: Content::Text {
        text: Cow::Borrowed(", "),
    },
    children: Cow::Borrowed(&[]),
    format: Format::new(),
    interactions: Interactivity::new(),
};
impl Resolvable {
    /// The grey comma vanilla puts between entities.
    #[must_use]
    pub const fn entity_separator() -> &'static TextComponent {
        &ENTITY_SEPARATOR
    }
    /// The comma vanilla puts between NBT values.
    #[must_use]
    pub const fn nbt_separator() -> &'static TextComponent {
        &NBT_SEPARATOR
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
/// Where a [`Resolvable::NBT`] reads its tag from.
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum NbtSource {
    Entity(Cow<'static, str>),
    Block(Cow<'static, str>),
    Storage(Cow<'static, str>),
}
impl NbtSource {
    /// Creates a [`NbtSource`] from a entity selector.
    pub fn entity<T: Into<Cow<'static, str>>>(selector: T) -> Self {
        NbtSource::Entity(selector.into())
    }
    /// Creates a [`NbtSource`] from a block coordinates.
    #[must_use]
    pub fn block(x: i32, y: i32, z: i32) -> Self {
        NbtSource::Block(Cow::Owned(format!("{x} {y} {z}")))
    }
    /// Creates a [`NbtSource`] from a Nbt Storage identifier.
    pub fn storage<T: Into<Cow<'static, str>>>(identifier: T) -> Self {
        NbtSource::Storage(identifier.into())
    }
}

impl From<Content> for TextComponent {
    fn from(value: Content) -> Self {
        TextComponent {
            content: value,
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}
impl From<Object> for TextComponent {
    fn from(value: Object) -> Self {
        TextComponent {
            content: Content::Object(value),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}
impl From<ObjectPlayer> for TextComponent {
    fn from(value: ObjectPlayer) -> Self {
        TextComponent {
            content: Content::Object(Object::Player {
                player: MaybeStatic::Owned(Box::new(value)),
                hat: true,
                fallback: None,
            }),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}
impl From<Resolvable> for TextComponent {
    fn from(value: Resolvable) -> Self {
        TextComponent {
            content: Content::Resolvable(value),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}
