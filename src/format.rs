use colored::{ColoredString, Colorize};
use std::{borrow::Cow, fmt::Display};

#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct Format {
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub color: Option<Color>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub font: Option<Cow<'static, str>>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub bold: Option<bool>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub italic: Option<bool>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub underlined: Option<bool>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub strikethrough: Option<bool>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub obfuscated: Option<bool>,
    #[cfg_attr(
        feature = "serde",
        serde(skip_serializing_if = "Option::is_none", default)
    )]
    pub shadow_color: Option<i32>,
}

impl Default for Format {
    fn default() -> Self {
        Self::new()
    }
}
impl Format {
    pub const fn new() -> Self {
        Self {
            color: None,
            font: None,
            bold: None,
            italic: None,
            underlined: None,
            strikethrough: None,
            obfuscated: None,
            shadow_color: None,
        }
    }
    pub fn is_none(&self) -> bool {
        self.color.is_none()
            && self.font.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
            && self.underlined.is_none()
            && self.strikethrough.is_none()
            && self.obfuscated.is_none()
            && self.shadow_color.is_none()
    }
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
    pub fn color_hex(mut self, color: &str) -> Self {
        if let Some(color) = Color::from_hex(color) {
            self.color = Some(color);
        }
        self
    }
    pub fn font<F: Into<Cow<'static, str>>>(mut self, font: F) -> Self {
        self.font = Some(font.into());
        self
    }
    pub fn bold(mut self, value: bool) -> Self {
        self.bold = Some(value);
        self
    }
    pub fn italic(mut self, value: bool) -> Self {
        self.italic = Some(value);
        self
    }
    pub fn underlined(mut self, value: bool) -> Self {
        self.underlined = Some(value);
        self
    }
    pub fn strikethrough(mut self, value: bool) -> Self {
        self.strikethrough = Some(value);
        self
    }
    pub fn obfuscated(mut self, value: bool) -> Self {
        self.obfuscated = Some(value);
        self
    }
    pub fn shadow_color(mut self, a: u8, r: u8, g: u8, b: u8) -> Self {
        self.shadow_color = Some(Self::parse_shadow_color(a, r, g, b));
        self
    }
    pub fn parse_shadow_color(a: u8, r: u8, g: u8, b: u8) -> i32 {
        (((a as u32) << 24) + ((r as u32) << 16) + ((g as u32) << 8) + (b as u32)) as i32
    }
    pub fn reset(mut self) -> Self {
        self.color = Some(Color::White);
        self.font = Some(Cow::Borrowed("minecraft:default"));
        self.bold = Some(false);
        self.italic = Some(false);
        self.underlined = Some(false);
        self.strikethrough = Some(false);
        self.obfuscated = Some(false);
        self.shadow_color = None;
        self
    }
    pub fn mix(&self, other: &Format) -> Format {
        Format {
            color: if self.color.is_some() {
                self.color.clone()
            } else {
                other.color.clone()
            },
            font: if self.font.is_some() {
                self.font.clone()
            } else {
                other.font.clone()
            },
            bold: if self.bold.is_some() {
                self.bold
            } else {
                other.bold
            },
            italic: if self.italic.is_some() {
                self.italic
            } else {
                other.italic
            },
            underlined: if self.underlined.is_some() {
                self.underlined
            } else {
                other.underlined
            },
            strikethrough: if self.strikethrough.is_some() {
                self.strikethrough
            } else {
                other.strikethrough
            },
            obfuscated: if self.obfuscated.is_some() {
                self.obfuscated
            } else {
                other.obfuscated
            },
            shadow_color: if self.shadow_color.is_some() {
                self.shadow_color
            } else {
                other.shadow_color
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Color {
    Aqua,
    Black,
    Blue,
    DarkAqua,
    DarkBlue,
    DarkGray,
    DarkGreen,
    DarkPurple,
    DarkRed,
    Gold,
    Gray,
    Green,
    LightPurple,
    Red,
    White,
    Yellow,
    Rgb(u8, u8, u8),
}
impl Color {
    pub fn from_hex(color: &str) -> Option<Color> {
        if color.starts_with('#')
            && color.chars().count() == 7
            && color[1..].find(|a: char| !a.is_ascii_hexdigit()).is_none()
        {
            let color = color.strip_prefix('#').unwrap();
            let (r, color) = color.split_at(2);
            let (g, b) = color.split_at(2);
            return Some(Color::Rgb(
                u8::from_str_radix(r, 16).unwrap(),
                u8::from_str_radix(g, 16).unwrap(),
                u8::from_str_radix(b, 16).unwrap(),
            ));
        }
        None
    }
    pub fn colorize_text<T: Into<String>>(&self, text: T) -> ColoredString {
        match self {
            Color::Black => text.into().black(),
            Color::DarkBlue => text.into().blue(),
            Color::DarkGreen => text.into().green(),
            Color::DarkAqua => text.into().cyan(),
            Color::DarkRed => text.into().red(),
            Color::DarkPurple => text.into().magenta(),
            Color::Gold => text.into().yellow(),
            Color::Gray => text.into().white(),
            Color::DarkGray => text.into().bright_black(),
            Color::Blue => text.into().bright_blue(),
            Color::Green => text.into().bright_green(),
            Color::Aqua => text.into().bright_cyan(),
            Color::Red => text.into().bright_red(),
            Color::LightPurple => text.into().bright_magenta(),
            Color::Yellow => text.into().bright_yellow(),
            Color::White => text.into().bright_white(),
            Color::Rgb(r, g, b) => text.into().truecolor(*r, *g, *b),
        }
    }
}
impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Color::Aqua => write!(f, "aqua"),
            Color::Black => write!(f, "black"),
            Color::Blue => write!(f, "blue"),
            Color::DarkAqua => write!(f, "dark_aqua"),
            Color::DarkBlue => write!(f, "dark_blue"),
            Color::DarkGray => write!(f, "dark_gray"),
            Color::DarkGreen => write!(f, "dark_green"),
            Color::DarkPurple => write!(f, "dark_purple"),
            Color::DarkRed => write!(f, "dark_red"),
            Color::Gold => write!(f, "gold"),
            Color::Gray => write!(f, "gray"),
            Color::Green => write!(f, "green"),
            Color::LightPurple => write!(f, "light_purple"),
            Color::Red => write!(f, "red"),
            Color::White => write!(f, "white"),
            Color::Yellow => write!(f, "yellow"),
            Color::Rgb(r, g, b) => write!(f, "#{:02X}{:02X}{:02X}", r, g, b),
        }
    }
}
