use colored::{ColoredString, Colorize};
use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};

/// The visual style of a component, each field unset unless given.
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
    /// Creates a [`Format`] with nothing set.
    #[must_use]
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
    /// Whether nothing is set.
    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.color.is_none()
            && self.font.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
            && self.underlined.is_none()
            && self.strikethrough.is_none()
            && self.obfuscated.is_none()
            && self.shadow_color.is_none()
    }
    /// Sets the color.
    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
    /// Sets the color from a `#RRGGBB` string, left unchanged when it does not parse.
    #[must_use]
    pub const fn color_hex(mut self, color: &str) -> Self {
        if let Some(color) = Color::from_hex(color) {
            self.color = Some(color);
        }
        self
    }
    /// Sets the font.
    #[must_use]
    pub const fn font<F: [const] Into<Cow<'static, str>>>(mut self, font: F) -> Self {
        self.font = Some(font.into());
        self
    }
    /// Sets bold.
    #[must_use]
    pub const fn bold(mut self, value: bool) -> Self {
        self.bold = Some(value);
        self
    }
    /// Sets italic.
    #[must_use]
    pub const fn italic(mut self, value: bool) -> Self {
        self.italic = Some(value);
        self
    }
    /// Sets underlined.
    #[must_use]
    pub const fn underlined(mut self, value: bool) -> Self {
        self.underlined = Some(value);
        self
    }
    /// Sets strikethrough.
    #[must_use]
    pub const fn strikethrough(mut self, value: bool) -> Self {
        self.strikethrough = Some(value);
        self
    }
    /// Sets obfuscated.
    #[must_use]
    pub const fn obfuscated(mut self, value: bool) -> Self {
        self.obfuscated = Some(value);
        self
    }
    /// Sets the text shadow from its alpha, red, green, and blue channels.
    #[must_use]
    pub const fn shadow_color(mut self, a: u8, r: u8, g: u8, b: u8) -> Self {
        self.shadow_color = Some(Self::parse_shadow_color(a, r, g, b));
        self
    }
    #[must_use]
    pub(crate) const fn parse_shadow_color(a: u8, r: u8, g: u8, b: u8) -> i32 {
        (((a as u32) << 24) + ((r as u32) << 16) + ((g as u32) << 8) + (b as u32)) as i32
    }
    pub(crate) const fn reset_in_place(&mut self) {
        self.color = Some(Color::White);
        self.font = Some(Cow::Borrowed("minecraft:default"));
        self.bold = Some(false);
        self.italic = Some(false);
        self.underlined = Some(false);
        self.strikethrough = Some(false);
        self.obfuscated = Some(false);
        self.shadow_color = None;
    }
    #[must_use]
    pub(crate) const fn reset(mut self) -> Self {
        self.reset_in_place();
        self
    }
    #[must_use]
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

/// A named text color or an arbitrary RGB one.
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
    /// The name this color serializes to in the component codec.
    #[must_use]
    pub fn codec_name(&self) -> Cow<'static, str> {
        match self {
            Color::Black => Cow::Borrowed("black"),
            Color::DarkBlue => Cow::Borrowed("dark_blue"),
            Color::DarkGreen => Cow::Borrowed("dark_green"),
            Color::DarkAqua => Cow::Borrowed("dark_aqua"),
            Color::DarkRed => Cow::Borrowed("dark_red"),
            Color::DarkPurple => Cow::Borrowed("dark_purple"),
            Color::Gold => Cow::Borrowed("gold"),
            Color::Gray => Cow::Borrowed("gray"),
            Color::DarkGray => Cow::Borrowed("dark_gray"),
            Color::Blue => Cow::Borrowed("blue"),
            Color::Green => Cow::Borrowed("green"),
            Color::Aqua => Cow::Borrowed("aqua"),
            Color::Red => Cow::Borrowed("red"),
            Color::LightPurple => Cow::Borrowed("light_purple"),
            Color::Yellow => Cow::Borrowed("yellow"),
            Color::White => Cow::Borrowed("white"),
            Color::Rgb(r, g, b) => Cow::Owned(format!("#{r:02X}{g:02X}{b:02X}")),
        }
    }

    /// The named color a codec name refers to.
    #[must_use]
    pub fn from_codec_name(name: &str) -> Option<Color> {
        Some(match name {
            "black" => Color::Black,
            "dark_blue" => Color::DarkBlue,
            "dark_green" => Color::DarkGreen,
            "dark_aqua" => Color::DarkAqua,
            "dark_red" => Color::DarkRed,
            "dark_purple" => Color::DarkPurple,
            "gold" => Color::Gold,
            "gray" => Color::Gray,
            "dark_gray" => Color::DarkGray,
            "blue" => Color::Blue,
            "green" => Color::Green,
            "aqua" => Color::Aqua,
            "red" => Color::Red,
            "light_purple" => Color::LightPurple,
            "yellow" => Color::Yellow,
            "white" => Color::White,
            _ => return None,
        })
    }

    /// The color a `#RRGGBB` string names, [None] when it does not parse.
    #[must_use]
    pub const fn from_hex(color: &str) -> Option<Color> {
        let bytes = color.as_bytes();
        if bytes.len() < 2 || bytes[0] != b'#' {
            return None;
        }

        let mut index = 1;
        let negative = match bytes[index] {
            b'-' => {
                index += 1;
                true
            }
            b'+' => {
                index += 1;
                false
            }
            _ => false,
        };
        if index == bytes.len() {
            return None;
        }

        let mut value: u32 = 0;
        while index < bytes.len() {
            let Some(digit) = hex_digit(bytes[index]) else {
                return None;
            };
            value = value * 16 + digit as u32;
            // vanilla range check and Integer.parseInt overflowing
            if value > 0x00FF_FFFF {
                return None;
            }
            index += 1;
        }
        if negative && value != 0 {
            return None;
        }

        Some(Color::Rgb(
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ))
    }
    pub(crate) fn colorize_text<T: Into<String>>(&self, text: T) -> ColoredString {
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
const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
impl Display for Color {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
            Color::Rgb(r, g, b) => write!(f, "#{r:02X}{g:02X}{b:02X}"),
        }
    }
}
