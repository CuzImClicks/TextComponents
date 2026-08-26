/// The UUID type `ShowEntity` and player heads carry.
pub use uuid::Uuid;

#[cfg(feature = "custom")]
use crate::custom::CustomData;
use crate::{EncodedNbt, TextComponent};
use std::{
    borrow::Cow,
    fmt::{self, Debug, Formatter},
    hash::{Hash, Hasher},
    ops::Deref,
};

/// A `'static` borrow or a heap-owned value.
pub enum MaybeStatic<T: 'static> {
    Static(&'static T),
    Owned(Box<T>),
}

impl<T> MaybeStatic<T> {
    #[must_use]
    pub(crate) const fn get(&self) -> &T {
        match self {
            Self::Static(value) => value,
            Self::Owned(value) => value,
        }
    }
}

impl<T> Deref for MaybeStatic<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.get()
    }
}

impl<T: Clone> Clone for MaybeStatic<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Static(value) => Self::Static(value),
            Self::Owned(value) => Self::Owned(value.clone()),
        }
    }
}

impl<T: Debug> Debug for MaybeStatic<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

impl<T: PartialEq> PartialEq for MaybeStatic<T> {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}
impl<T: Eq> Eq for MaybeStatic<T> {}

impl<T: Hash> Hash for MaybeStatic<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get().hash(state);
    }
}

impl<T> From<T> for MaybeStatic<T> {
    fn from(value: T) -> Self {
        Self::Owned(Box::new(value))
    }
}

#[cfg(feature = "serde")]
impl<T: ::serde::Serialize> ::serde::Serialize for MaybeStatic<T> {
    fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.get().serialize(s)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: ::serde::Deserialize<'de>> ::serde::Deserialize<'de> for MaybeStatic<T> {
    fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(Self::from)
    }
}

/// The click behavior, hover behavior, and chat insertion of a component.
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct Interactivity {
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub insertion: Option<Cow<'static, str>>,
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "Option::is_none",
            rename = "click_event",
            default
        )
    )]
    pub click: Option<MaybeStatic<ClickEvent>>,
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "Option::is_none",
            rename = "hover_event",
            default
        )
    )]
    pub hover: Option<MaybeStatic<HoverEvent>>,
}

impl Default for Interactivity {
    fn default() -> Self {
        Self::new()
    }
}

impl Interactivity {
    /// Creates an [`Interactivity`] with nothing set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            insertion: None,
            click: None,
            hover: None,
        }
    }
    /// Whether nothing is set.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.insertion.is_none() && self.click.is_none() && self.hover.is_none()
    }
    pub(crate) fn mix(&self, other: &mut Self) {
        if self.insertion.is_some() {
            other.insertion.clone_from(&self.insertion);
        }
        if self.click.is_some() {
            other.click.clone_from(&self.click);
        }
        if self.hover.is_some() {
            other.hover.clone_from(&self.hover);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
/// What happens when a component is clicked.
#[cfg_attr(feature = "serde", serde(tag = "action", rename_all = "snake_case"))]
pub enum ClickEvent {
    OpenUrl {
        url: Cow<'static, str>,
    },
    RunCommand {
        command: Cow<'static, str>,
    },
    SuggestCommand {
        command: Cow<'static, str>,
    },
    ChangePage {
        page: i32,
    },
    CopyToClipboard {
        value: Cow<'static, str>,
    },
    ShowDialog {
        dialog: Dialog,
    },
    #[cfg(feature = "custom")]
    Custom(CustomData),
}
impl ClickEvent {
    /// Creates a [`ClickEvent`] that opens a url when triggered.
    pub fn open_url<T: Into<Cow<'static, str>>>(url: T) -> Self {
        ClickEvent::OpenUrl { url: url.into() }
    }
    /// Creates a [`ClickEvent`] that runs a command when triggered.
    pub fn run_command<T: Into<Cow<'static, str>>>(command: T) -> Self {
        ClickEvent::RunCommand {
            command: command.into(),
        }
    }
    /// Creates a [`ClickEvent`] that replaces the chat input with a command when triggered.
    pub fn suggest_command<T: Into<Cow<'static, str>>>(command: T) -> Self {
        ClickEvent::SuggestCommand {
            command: command.into(),
        }
    }
    /// Creates a [`ClickEvent`] that changes the page of a book when triggered.
    #[must_use]
    pub const fn change_page(page: u32) -> Self {
        ClickEvent::ChangePage { page: page as i32 }
    }
    /// Creates a [`ClickEvent`] that copies it's content to the clipboard when triggered.
    pub fn copy_to_clipboard<T: Into<Cow<'static, str>>>(value: T) -> Self {
        ClickEvent::CopyToClipboard {
            value: value.into(),
        }
    }
    /// Creates a [`ClickEvent`] that shows a custom dialog when triggered.
    pub fn show_dialog<T: Into<Cow<'static, str>>>(dialog: T) -> Self {
        ClickEvent::ShowDialog {
            dialog: Dialog::Reference(dialog.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
/// The dialog a [`ClickEvent::ShowDialog`] opens.
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Dialog {
    /// The id of a dialog in the registry.
    Reference(Cow<'static, str>),
    /// A dialog definition carried in the component.
    Inline(EncodedNbt),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
/// What is shown when a component is hovered.
#[cfg_attr(feature = "serde", serde(tag = "action", rename_all = "snake_case"))]
pub enum HoverEvent {
    ShowText {
        value: MaybeStatic<TextComponent>,
    },
    ShowItem {
        id: Cow<'static, str>,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "is_one", default = "one")
        )]
        count: i32,
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Option::is_none", default)
        )]
        components: Option<EncodedNbt>,
    },
    ShowEntity {
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Option::is_none", default)
        )]
        name: Option<MaybeStatic<TextComponent>>,
        id: Cow<'static, str>,
        uuid: Uuid,
    },
}
impl HoverEvent {
    /// Creates a [`HoverEvent`] that will show a text component.
    pub fn show_text<T: Into<TextComponent>>(text: T) -> Self {
        HoverEvent::ShowText {
            value: MaybeStatic::Owned(Box::new(text.into())),
        }
    }
    /// Creates a [`HoverEvent`] that will show an item.
    pub fn show_item<T: Into<Cow<'static, str>>>(
        id: T,
        count: Option<i32>,
        components: Option<EncodedNbt>,
    ) -> Self {
        HoverEvent::ShowItem {
            id: id.into(),
            count: count.unwrap_or(1),
            components: components.filter(|value| !value.is_empty_compound()),
        }
    }
    /// Creates a [`HoverEvent`] that will show an entity.
    pub fn show_entity<T: Into<Cow<'static, str>>, R: Into<TextComponent>>(
        id: T,
        uuid: Uuid,
        name: Option<R>,
    ) -> Self {
        HoverEvent::ShowEntity {
            name: name.map(|r| MaybeStatic::Owned(Box::new(r.into()))),
            id: id.into(),
            uuid,
        }
    }
}

#[cfg(feature = "serde")]
const fn one() -> i32 {
    1
}

#[cfg(feature = "serde")]
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde `skip_serializing_if` needs `fn(&T) -> bool`"
)]
const fn is_one(value: &i32) -> bool {
    *value == 1
}
