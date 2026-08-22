//! A Rust implementation of Minecraft's text components.
#![feature(const_trait_impl)]
#![feature(const_convert)]
#![feature(const_destruct)]
#![feature(const_option_ops)]
#![feature(const_default)]
#[cfg(feature = "custom")]
use crate::custom::CustomContent;
use crate::{
    content::{Content, NbtSource, Object, ObjectPlayer, Resolvable},
    format::{Color, Format},
    interactivity::{ClickEvent, HoverEvent, Interactivity, MaybeStatic},
    translation::TranslatedMessage,
};
use std::borrow::Cow;

/// Generates translation constants from a language file.
#[cfg(feature = "build")]
pub mod build;
/// What a component displays.
pub mod content;
/// Content this crate renders through a registry supplied by the caller.
#[cfg(feature = "custom")]
pub mod custom;
mod encoded_nbt;
/// Plain and colored terminal output.
pub mod fmt;
/// Colors and text styling.
pub mod format;
/// Click events, hover events, and chat insertions.
pub mod interactivity;
#[cfg(feature = "minimessage")]
pub mod minimessage;
/// Network NBT encoding and decoding.
#[cfg(feature = "nbt")]
pub mod nbt;
mod nbt_value;
pub(crate) mod parse;
/// Filling in content the server has to look up.
pub mod resolving;
/// Translation keys and their arguments.
pub mod translation;

pub use encoded_nbt::{EmbeddedNbtCodec, EncodedComponent, EncodedNbt};
#[cfg(feature = "nbt")]
pub use nbt::SpliceComponent;
pub use nbt_value::NbtValue;
pub use parse::{SnbtError, SnbtResult};
pub use translation::Args;

/// The encode helpers `text_nbt!` expands to calls of.
#[cfg(feature = "nbt")]
#[doc(hidden)]
pub mod __private {
    pub use crate::nbt::{
        Mutf8Fmt, SplicePart, nbt_len, splice_bytes, splice_len, write_color, write_component_elem,
        write_component_entry, write_component_root, write_mutf8, write_mutf8_body,
    };

    /// No-op branch of the `already_encoded` hint.
    pub trait AlreadyEncoded {
        fn already_encoded(&self) {}
    }

    impl<T: ?Sized> AlreadyEncoded for T {}
}
/// `text!` and `text_nbt!`: `MiniMessage` templates checked at compile time.
/// Same syntax as [`minimessage`].
#[cfg(feature = "macros")]
pub use text_components_macros::{text, text_nbt};

/// A tree of styled text with click and hover events.
///
/// ### Styling
/// [`Style`] is implemented for any [`Into<TextComponent>`]:
/// ```
/// # use text_components::{TextComponent, Style, format::Color};
/// TextComponent::plain("Plain text").color(Color::Red);
/// "String slice".bold(true);
/// "Hex".color_hex("#bf00ff");
/// ```
/// ### Interactivity
/// ```
/// # use text_components::{Style, Modifier, interactivity::{ClickEvent, HoverEvent}};
/// "text"
///     .insertion("Inserted into chat on Shift+Click")
///     .hover_event(HoverEvent::show_text("Shown on hover"))
///     .click_event(ClickEvent::open_url("https://www.minecraft.net/"));
/// ```
/// ### Children
/// ```
/// # use text_components::{TextComponent, Style, Modifier};
/// TextComponent::new()
///     .add_child("Child 1")
///     .add_children(vec!["Child 2".color_hex("#bf00ff"), "Child 3".italic(true)]);
/// ```
/// ### Display
/// `{}` prints plain text, `{:p}` prints colored terminal text.
/// ### Building
/// Sending needs a [`TextResolutor`](resolving::TextResolutor) and a [`BuildTarget`](resolving::BuildTarget):
/// ```
/// # use text_components::{TextComponent, resolving::NoResolutor, fmt::TextBuilder};
/// # let resolutor = NoResolutor;
/// let component = TextComponent::plain("Component to build");
/// component.build(&resolutor, TextBuilder);
/// // shorthands
/// component.to_plain(&resolutor);
/// component.to_pretty(&resolutor);
/// ```
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct TextComponent {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub content: Content,
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "<[TextComponent]>::is_empty",
            rename = "extra",
            default
        )
    )]
    pub children: Cow<'static, [TextComponent]>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub format: Format,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub interactions: Interactivity,
}

impl TextComponent {
    /// Creates an empty [`TextComponent`], useful to make it the parent.
    #[must_use]
    pub const fn new() -> Self {
        TextComponent {
            content: Content::Text {
                text: Cow::Borrowed(""),
            },
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }

    /// Creates a [`TextComponent`] of a plain text at compile time.
    /// ## Example
    /// ```
    /// # use text_components::TextComponent;
    /// const TEST: TextComponent = TextComponent::const_plain("Test Component");
    /// ```
    #[must_use]
    pub const fn const_plain(text: &'static str) -> Self {
        TextComponent {
            content: Content::Text {
                text: Cow::Borrowed(text),
            },
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }

    /// Creates a [`TextComponent`] of a plain text.
    /// ## Example
    /// ```
    /// # use text_components::TextComponent;
    /// let component = TextComponent::plain("Test Component");
    /// ```
    /// This is equivalent of doing:
    /// ```
    /// # use text_components::TextComponent;
    /// let component: TextComponent = "Test Component".into();
    /// ```
    pub const fn plain<T: [const] Into<Cow<'static, str>>>(text: T) -> Self {
        TextComponent {
            content: Content::Text { text: text.into() },
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }

    /// Creates a [`TextComponent`] of a [`TranslatedMessage`], it's recommended using a compiled
    /// [Translation](crate::translation::Translation) which forces you to give it the right amount of arguments.
    /// ## Examples
    /// ```
    /// # use text_components::TextComponent;
    /// # use text_components::translation::Translation;
    /// const ITEM_MINECRAFT_DIAMOND_SWORD: Translation<0> = Translation::<0>("item.minecraft.diamond_sword");
    ///
    /// // Results in "Diamond Sword"
    /// TextComponent::translated(ITEM_MINECRAFT_DIAMOND_SWORD.msg());
    /// // This is equivalent of doing:
    /// let component: TextComponent = (&ITEM_MINECRAFT_DIAMOND_SWORD).into();
    /// // or
    /// ITEM_MINECRAFT_DIAMOND_SWORD.msg().component();
    ///
    /// // For a translation with 2 arguments:
    ///
    /// // Results in "The Rust compiler was killed by you using magic".
    /// TextComponent::translated(Translation::<2>("death.attack.indirect_magic").message(["The Rust compiler", "you"]));
    /// ```
    #[must_use]
    pub const fn translated(message: TranslatedMessage) -> Self {
        TextComponent {
            content: Content::Translate(message),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }

    /// Creates a [`TextComponent`] with an image from a resource pack in it.\
    /// * `sprite` - The path to the texture, starting from the atlas\
    /// * `atlas` - The atlas where the texture belongs, if it's [None] will default to "minecraft:blocks"
    /// ## Example
    /// ```
    /// # use text_components::TextComponent;
    /// // Displays the Diamond Sword sprite
    /// TextComponent::atlas("item/diamond_sword", Some("minecraft:items"));
    /// ```
    pub const fn atlas<T: [const] Into<Cow<'static, str>>, R: [const] Into<Cow<'static, str>>>(
        sprite: T,
        atlas: Option<R>,
    ) -> Self {
        TextComponent {
            content: Content::Object(Object::Atlas {
                atlas: atlas.map_or(Cow::Borrowed("minecraft:blocks"), Into::into),
                sprite: sprite.into(),
                fallback: None,
            }),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
    /// Creates a [`TextComponent`] with the head of a player in it.
    /// * `player` - A [`ObjectPlayer`] containing the required info
    /// * `hat` - Whether to display the hat layer
    /// ## Example
    /// ```
    /// # use text_components::{TextComponent, content::ObjectPlayer};
    /// // Displays the head of Jeb_
    /// TextComponent::player_head(ObjectPlayer::name("Jeb_"), true);
    /// ```
    #[must_use]
    pub fn player_head(player: ObjectPlayer, hat: bool) -> Self {
        TextComponent {
            content: Content::Object(Object::Player {
                player: Box::new(player),
                hat,
                fallback: None,
            }),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }

    /// Creates a [`TextComponent`] that will contain the value of a Scoreboard.
    /// * `selector` - Describes the player to get the data (Needs to be only 1 entity)\
    ///   The character '*' can be used to show the receiver player data
    /// * `objective` - The internal name of the scoreboard to show.
    /// #### Needs [`TextComponent::resolve`] before being sent.
    /// ## Example
    /// ```
    /// # use text_components::TextComponent;
    /// // Displays the 'deaths' scoreboard value of the nearest player
    /// TextComponent::scoreboard("@p", "deaths");
    /// ```
    pub const fn scoreboard<
        T: [const] Into<Cow<'static, str>>,
        R: [const] Into<Cow<'static, str>>,
    >(
        selector: T,
        objective: R,
    ) -> Self {
        TextComponent {
            content: Content::Resolvable(Resolvable::Scoreboard {
                selector: selector.into(),
                objective: objective.into(),
            }),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }

    /// Creates a [`TextComponent`] containing an entity or group of entities.
    /// * `selector` - The selector of the entities to display
    /// * `separator` - The component separating multiple entities. If [None] will be a grey comma
    /// #### Needs [`TextComponent::resolve`] before being sent.
    /// ## Example
    /// ```
    /// # use text_components::TextComponent;
    /// // Displays all the players name separated by a space
    /// TextComponent::entity("@a", Some(" ".into()));
    /// ```
    pub fn entity<T: Into<Cow<'static, str>>>(selector: T, separator: Option<Self>) -> Self {
        TextComponent {
            content: Content::Resolvable(Resolvable::Entity {
                selector: selector.into(),
                separator: separator.map(Box::new),
            }),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }

    /// Creates a [`TextComponent`] containing the data of a Nbt tag.
    /// * `path` - The Nbt path of the tag to show
    /// * `source` - A [`NbtSource`] indicating where to search the nbt tag
    /// * `interpret` - If [true](bool) the Nbt data will be read as it's a text component
    /// * `separator` - The component separating multiple Nbt tags. If [None] will be a comma
    /// #### Needs [`TextComponent::resolve`] before being sent.
    /// ## Example
    /// ```
    /// # use text_components::{TextComponent, content::NbtSource};
    /// // Displays the nearest player health
    /// TextComponent::nbt("Health", NbtSource::entity("@p"), false, None);
    /// ```
    pub fn nbt<T: Into<Cow<'static, str>>>(
        path: T,
        source: NbtSource,
        interpret: bool,
        separator: Option<Self>,
    ) -> Self {
        TextComponent {
            content: Content::Resolvable(Resolvable::NBT {
                path: path.into(),
                interpret,
                plain: false,
                separator: separator.map(Box::new),
                source,
            }),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }

    /// Creates a [`TextComponent`] holding registered custom content.
    #[cfg(feature = "custom")]
    pub fn custom<T: CustomContent>(content: T) -> TextComponent {
        TextComponent {
            content: Content::Custom(content.as_data()),
            children: Cow::Borrowed(&[]),
            format: Format::new(),
            interactions: Interactivity::new(),
        }
    }
}

#[expect(
    clippy::missing_const_for_fn,
    reason = "this is a const impl, they are already const"
)]
const impl TextComponent {
    /// A const node.
    #[must_use]
    pub fn const_node(
        text: &'static str,
        children: &'static [TextComponent],
        click: Option<&'static ClickEvent>,
        hover: Option<&'static HoverEvent>,
    ) -> Self {
        TextComponent {
            content: Content::Text {
                text: Cow::Borrowed(text),
            },
            children: Cow::Borrowed(children),
            format: Format::new(),
            interactions: Interactivity {
                insertion: None,
                click: match click {
                    Some(event) => Some(MaybeStatic::Static(event)),
                    None => None,
                },
                hover: match hover {
                    Some(event) => Some(MaybeStatic::Static(event)),
                    None => None,
                },
            },
        }
    }

    /// Shorthand for a node with only borrowed children.
    #[must_use]
    pub fn const_tree(text: &'static str, children: &'static [TextComponent]) -> Self {
        Self::const_node(text, children, None, None)
    }
}

const impl Default for TextComponent {
    fn default() -> Self {
        TextComponent::new()
    }
}

const impl From<&'static str> for TextComponent {
    fn from(value: &'static str) -> Self {
        TextComponent::const_plain(value)
    }
}
impl From<String> for TextComponent {
    fn from(value: String) -> Self {
        TextComponent::plain(value)
    }
}

/// Children and events on anything that converts into a [`TextComponent`].
pub trait Modifier {
    type Output;
    /// Adds a child at the end of a text component
    fn add_child<T: Into<TextComponent>>(self, child: T) -> Self::Output;
    /// Appends a [vec] of [Into]<[`TextComponent`]> as children of this component
    fn add_children<T: Into<TextComponent>>(self, children: Vec<T>) -> Self::Output;
    /// Sets the [`ClickEvent`] for this component
    fn click_event(self, click: ClickEvent) -> Self::Output;
    /// Sets the [`HoverEvent`] for this component
    fn hover_event(self, hover: HoverEvent) -> Self::Output;
}

/// Formatting on anything that converts into a [`TextComponent`].
pub const trait Style {
    type Output;
    /// Sets the [Color] of this component
    fn color(self, color: Color) -> Self::Output;
    /// Sets the color of this component from a 6 digit hex color
    fn color_hex(self, color: &str) -> Self::Output;
    /// Sets the Shift+Click chat insertion string
    fn insertion<T: [const] Into<Cow<'static, str>>>(self, insertion: T) -> Self::Output;
    /// Makes this component **bold**
    fn bold(self, value: bool) -> Self::Output;
    /// Makes this component *italic*
    fn italic(self, value: bool) -> Self::Output;
    /// Makes this component underlined
    fn underlined(self, value: bool) -> Self::Output;
    /// Makes this component ~~strikethrough~~
    fn strikethrough(self, value: bool) -> Self::Output;
    /// Makes this component obfuscated
    fn obfuscated(self, value: bool) -> Self::Output;
    /// Sets the shadow color of this component
    fn shadow_color(self, a: u8, r: u8, g: u8, b: u8) -> Self::Output;
    /// Sets the font used to display this component
    fn font<F: [const] Into<Cow<'static, str>>>(self, font: F) -> Self::Output;
    /// Sets all the format of this component to the default
    fn reset(self) -> Self::Output;
}

const impl<T: [const] Into<TextComponent> + Sized> Style for T {
    type Output = TextComponent;

    fn color(self, color: Color) -> Self::Output {
        let mut component = self.into();
        component.format = component.format.color(color);
        component
    }
    fn color_hex(self, color: &str) -> TextComponent {
        let mut component = self.into();
        component.format = component.format.color_hex(color);
        component
    }
    fn insertion<R: [const] Into<Cow<'static, str>>>(self, insertion: R) -> Self::Output {
        let mut component = self.into();
        component.interactions.insertion = Some(insertion.into());
        component
    }
    fn bold(self, value: bool) -> TextComponent {
        let mut component = self.into();
        component.format = component.format.bold(value);
        component
    }
    fn italic(self, value: bool) -> TextComponent {
        let mut component = self.into();
        component.format = component.format.italic(value);
        component
    }
    fn underlined(self, value: bool) -> TextComponent {
        let mut component = self.into();
        component.format = component.format.underlined(value);
        component
    }
    fn strikethrough(self, value: bool) -> TextComponent {
        let mut component = self.into();
        component.format = component.format.strikethrough(value);
        component
    }
    fn obfuscated(self, value: bool) -> TextComponent {
        let mut component = self.into();
        component.format = component.format.obfuscated(value);
        component
    }
    fn shadow_color(self, a: u8, r: u8, g: u8, b: u8) -> TextComponent {
        let mut component = self.into();
        component.format = component.format.shadow_color(a, r, g, b);
        component
    }
    fn font<F: [const] Into<Cow<'static, str>>>(self, font: F) -> Self::Output {
        let mut component = self.into();
        component.format = component.format.font(font);
        component
    }
    fn reset(self) -> TextComponent {
        let mut component = self.into();
        component.format = component.format.reset();
        component
    }
}

impl<T: Into<TextComponent> + Sized> Modifier for T {
    type Output = TextComponent;
    fn add_child<F: Into<TextComponent>>(self, child: F) -> TextComponent {
        let mut component = self.into();
        component.children.to_mut().push(child.into());
        component
    }
    fn add_children<F: Into<TextComponent>>(self, children: Vec<F>) -> TextComponent {
        let mut component = self.into();
        for child in children {
            component.children.to_mut().push(child.into());
        }
        component
    }
    fn click_event(self, click: ClickEvent) -> TextComponent {
        let mut component = self.into();
        component.interactions.click = Some(MaybeStatic::Owned(Box::new(click)));
        component
    }
    fn hover_event(self, hover: HoverEvent) -> TextComponent {
        let mut component = self.into();
        component.interactions.hover = Some(MaybeStatic::Owned(Box::new(hover)));
        component
    }
}

const impl<'a> Style for &'a mut TextComponent {
    type Output = &'a mut TextComponent;

    fn color(self, color: Color) -> Self::Output {
        self.format.color = Some(color);
        self
    }
    fn color_hex(self, color: &str) -> Self::Output {
        if let Some(hex) = Color::from_hex(color) {
            self.format.color = Some(hex);
        }
        self
    }

    fn insertion<T: [const] Into<Cow<'static, str>>>(self, insertion: T) -> Self::Output {
        self.interactions.insertion = Some(insertion.into());
        self
    }

    fn bold(self, value: bool) -> Self::Output {
        self.format.bold = Some(value);
        self
    }

    fn italic(self, value: bool) -> Self::Output {
        self.format.italic = Some(value);
        self
    }

    fn underlined(self, value: bool) -> Self::Output {
        self.format.underlined = Some(value);
        self
    }

    fn strikethrough(self, value: bool) -> Self::Output {
        self.format.strikethrough = Some(value);
        self
    }

    fn obfuscated(self, value: bool) -> Self::Output {
        self.format.obfuscated = Some(value);
        self
    }

    fn font<F: [const] Into<Cow<'static, str>>>(self, font: F) -> Self::Output {
        self.format.font = Some(font.into());
        self
    }

    fn shadow_color(self, a: u8, r: u8, g: u8, b: u8) -> Self::Output {
        self.format.shadow_color = Some(Format::parse_shadow_color(a, r, g, b));
        self
    }

    fn reset(self) -> Self::Output {
        self.format.reset_in_place();
        self
    }
}

impl<'a> Modifier for &'a mut TextComponent {
    type Output = &'a mut TextComponent;
    fn add_child<T: Into<TextComponent>>(self, child: T) -> &'a mut TextComponent {
        self.children.to_mut().push(child.into());
        self
    }

    fn add_children<T: Into<TextComponent>>(self, children: Vec<T>) -> &'a mut TextComponent {
        for child in children {
            self.children.to_mut().push(child.into());
        }
        self
    }

    fn click_event(self, click: ClickEvent) -> &'a mut TextComponent {
        self.interactions.click = Some(MaybeStatic::Owned(Box::new(click)));
        self
    }

    fn hover_event(self, hover: HoverEvent) -> &'a mut TextComponent {
        self.interactions.hover = Some(MaybeStatic::Owned(Box::new(hover)));
        self
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;
    use crate::translation::TranslatedMessage;
    use std::mem::size_of;

    #[test]
    fn model_widths_are_unchanged() {
        assert_eq!(size_of::<TranslatedMessage>(), 72);
        assert_eq!(size_of::<Content>(), 80);
        assert_eq!(size_of::<TextComponent>(), 208);
    }
}
