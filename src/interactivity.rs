use uuid::Uuid;

#[cfg(feature = "custom")]
use crate::custom::CustomData;
use crate::{NbtValue, TextComponent};
use std::borrow::Cow;

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
    pub click: Option<ClickEvent>,
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "Option::is_none",
            rename = "hover_event",
            default
        )
    )]
    pub hover: Option<HoverEvent>,
}

impl Default for Interactivity {
    fn default() -> Self {
        Self::new()
    }
}

impl Interactivity {
    pub const fn new() -> Self {
        Self {
            insertion: None,
            click: None,
            hover: None,
        }
    }
    pub fn is_none(&self) -> bool {
        self.insertion.is_none() && self.click.is_none() && self.hover.is_none()
    }
    pub fn mix(&self, other: &mut Self) {
        if self.insertion.is_some() {
            other.insertion = self.insertion.clone()
        }
        if self.click.is_some() {
            other.click = self.click.clone()
        }
        if self.hover.is_some() {
            other.hover = self.hover.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
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
    /// Creates a [ClickEvent] that opens a url when triggered.
    pub fn open_url<T: Into<Cow<'static, str>>>(url: T) -> Self {
        ClickEvent::OpenUrl { url: url.into() }
    }
    /// Creates a [ClickEvent] that runs a command when triggered.
    pub fn run_command<T: Into<Cow<'static, str>>>(command: T) -> Self {
        ClickEvent::RunCommand {
            command: command.into(),
        }
    }
    /// Creates a [ClickEvent] that replaces the chat input with a command when triggered.
    pub fn suggest_command<T: Into<Cow<'static, str>>>(command: T) -> Self {
        ClickEvent::SuggestCommand {
            command: command.into(),
        }
    }
    /// Creates a [ClickEvent] that changes the page of a book when triggered.
    pub fn change_page(page: u32) -> Self {
        ClickEvent::ChangePage { page: page as i32 }
    }
    /// Creates a [ClickEvent] that copies it's content to the clipboard when triggered.
    pub fn copy_to_clipboard<T: Into<Cow<'static, str>>>(value: T) -> Self {
        ClickEvent::CopyToClipboard {
            value: value.into(),
        }
    }
    /// Creates a [ClickEvent] that shows a custom dialog when triggered.
    /// * `dialog` - Either a dialog id or a dialog definition
    pub fn show_dialog<T: Into<Cow<'static, str>>>(dialog: T) -> Self {
        ClickEvent::ShowDialog {
            dialog: Dialog::Reference(dialog.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Dialog {
    Reference(Cow<'static, str>),
    Inline(NbtValue),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "action", rename_all = "snake_case"))]
pub enum HoverEvent {
    ShowText {
        value: Box<TextComponent>,
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
        components: Option<NbtValue>,
    },
    ShowEntity {
        #[cfg_attr(
            feature = "serde",
            serde(skip_serializing_if = "Option::is_none", default)
        )]
        name: Option<Box<TextComponent>>,
        id: Cow<'static, str>,
        uuid: Uuid,
    },
}
impl HoverEvent {
    /// Creates a [HoverEvent] that will show a text component.
    pub fn show_text<T: Into<TextComponent>>(text: T) -> Self {
        HoverEvent::ShowText {
            value: Box::new(text.into()),
        }
    }
    /// Creates a [HoverEvent] that will show an item.
    /// * `id` - The id of the item
    /// * `count` - If [Some] shows the amount of items
    /// * `components` - An optional data component patch
    pub fn show_item<T: Into<Cow<'static, str>>>(
        id: T,
        count: Option<i32>,
        components: Option<NbtValue>,
    ) -> Self {
        HoverEvent::ShowItem {
            id: id.into(),
            count: count.unwrap_or(1),
            components: components.filter(|value| !value.is_empty_compound()),
        }
    }
    /// Creates a [HoverEvent] that will show an entity.
    /// * `id` - The id of the entity's type
    /// * `uuid` - The id of the targeted entity
    /// * `name` - If [Some] the name to display
    pub fn show_entity<T: Into<Cow<'static, str>>, R: Into<TextComponent>>(
        id: T,
        uuid: Uuid,
        name: Option<R>,
    ) -> Self {
        HoverEvent::ShowEntity {
            name: name.map(|r| Box::new(r.into())),
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
const fn is_one(value: &i32) -> bool {
    *value == 1
}
