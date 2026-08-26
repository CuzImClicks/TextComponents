#[derive(Debug, Clone, PartialEq)]
pub enum HoleArg {
    /// `{}` — filled from macro arguments in order.
    Positional(usize),
    /// `{name}` — filled by name.
    Named {
        name: String,
        /// Char range of the hole in the outermost template.
        range: (usize, usize),
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoleKind {
    /// `{}` — a string.
    Text,
    /// `{@}` — a component.
    Component,
    /// `{@const NAME}` — a `const` component whose bytes are folded in at compile time.
    ConstComponent,
    /// `{const NAME}` — a `const &str` spliced as literal text.
    ConstText,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColorIr {
    /// A named color, canonicalized (`grey` → `gray`).
    Named(&'static str),
    /// `<#RRGGBB>`.
    Rgb(u8, u8, u8),
    /// `<{}>` / `<{name}>` — a color chosen at runtime.
    Dyn(HoleArg),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoolIr {
    Const(bool),
    /// `<b:{}>` / `<b:{is_op}>` — a flag chosen at runtime.
    Dyn(HoleArg),
}

/// One part of a click value: `'/msg {} '` parses to [Lit, Hole, Lit].
#[derive(Debug, Clone, PartialEq)]
pub enum StrSeg {
    Lit(String),
    /// A hole with an optional `format!` spec (`{x:.2}` → `Some(".2")`).
    Hole(HoleArg, Option<String>),
}

impl StrSeg {
    /// Joins segments that are all literals; None if any hole is present.
    #[must_use]
    pub fn join_lits(segs: &[StrSeg]) -> Option<String> {
        let mut out = String::new();
        for seg in segs {
            match seg {
                StrSeg::Lit(lit) => out.push_str(lit),
                StrSeg::Hole(..) => return None,
            }
        }
        Some(out)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClickKind {
    OpenUrl,
    RunCommand,
    SuggestCommand,
    CopyToClipboard,
    ChangePage(i32),
    ShowDialog,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HoverIr {
    /// `<hover:show_text:'…'>` — full markup, holes included.
    Text(Vec<Piece>),
    /// `<hover:show_item:'minecraft:diamond_sword':3>`.
    Item { id: String, count: i32 },
    /// `<hover:show_entity:'minecraft:pig':uuid:'name'>` — the name its own template.
    Entity {
        id: String,
        /// Big-endian bytes, as `Uuid::as_bytes`.
        uuid: [u8; 16],
        name: Option<Vec<Piece>>,
    },
    /// `<hover:{item}>` — a whole `HoverEvent` supplied at runtime.
    Dyn(HoleArg),
}

/// Where `<nbt:…>` reads from.
#[derive(Debug, Clone, PartialEq)]
pub enum NbtSourceIr {
    Block(String),
    Entity(String),
    Storage(String),
}

/// How `<head:…>` names the player.
#[derive(Debug, Clone, PartialEq)]
pub enum HeadIr {
    Name(String),
    /// Big-endian bytes, as `Uuid::as_bytes`.
    Uuid([u8; 16]),
    Texture(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClickIr {
    /// The value may contain `{}` / `{name}` string holes.
    Action(ClickKind, Vec<StrSeg>),
    /// `<click:{action}>` — a whole `ClickEvent` supplied at runtime.
    Dyn(HoleArg),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub color: Option<ColorIr>,
    /// `<font:minecraft:uniform>` — a resource location, passed through.
    pub font: Option<String>,
    pub bold: Option<BoolIr>,
    pub italic: Option<BoolIr>,
    pub underlined: Option<BoolIr>,
    pub strikethrough: Option<BoolIr>,
    pub obfuscated: Option<BoolIr>,
    /// `<shadow:#AARRGGBB>` — packed ARGB, as the codec stores it.
    pub shadow_color: Option<i32>,
    /// `<insertion:'…'>` — shift-click chat text, string holes included.
    pub insertion: Option<Vec<StrSeg>>,
    pub hover: Option<HoverIr>,
    pub click: Option<ClickIr>,
}

impl Style {
    #[must_use]
    pub const fn flags(&self) -> [(&'static str, &Option<BoolIr>); 5] {
        [
            ("bold", &self.bold),
            ("italic", &self.italic),
            ("underlined", &self.underlined),
            ("strikethrough", &self.strikethrough),
            ("obfuscated", &self.obfuscated),
        ]
    }

    /// Whether any part of this style is decided at runtime.
    #[must_use]
    pub fn has_dyn(&self) -> bool {
        matches!(self.color, Some(ColorIr::Dyn(_)))
            || self
                .flags()
                .iter()
                .any(|(_, f)| matches!(f, Some(BoolIr::Dyn(_))))
            || self.insertion.as_ref().is_some_and(|s| segs_have_dyn(s))
            || match &self.hover {
                Some(HoverIr::Text(pieces)) => pieces_have_dyn(pieces),
                Some(HoverIr::Entity { name, .. }) => name.as_deref().is_some_and(pieces_have_dyn),
                Some(HoverIr::Dyn(_)) => true,
                Some(HoverIr::Item { .. }) | None => false,
            }
            || match &self.click {
                Some(ClickIr::Action(_, segs)) => segs_have_dyn(segs),
                Some(ClickIr::Dyn(_)) => true,
                None => false,
            }
    }
}

/// Whether a string value needs runtime assembly.
#[must_use]
pub fn segs_have_dyn(segs: &[StrSeg]) -> bool {
    segs.iter().any(|s| matches!(s, StrSeg::Hole(..)))
}

/// Whether any piece is a hole or carries runtime styling.
pub fn pieces_have_dyn(pieces: &[Piece]) -> bool {
    pieces.iter().any(Piece::has_dyn)
}

/// The key of a `<lang:…>` tag.
#[derive(Debug, Clone, PartialEq)]
pub enum LangKey {
    /// `<lang:multiplayer.player.left:…>` — the key spelled out.
    Lit(String),
    /// `<lang:{}:…>` / `<lang:{msg}:…>` — the key supplied at runtime.
    Dyn(HoleArg),
}

/// One part of a template, with the char range it occupies in the outermost template.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Text {
        text: String,
        style: Style,
        range: (usize, usize),
    },
    /// `<key:key.jump>` — the client renders the player's own binding.
    Keybind {
        key: String,
        style: Style,
        range: (usize, usize),
    },
    /// `<lang:key:arg:arg>` — a translation the client resolves, each arg its own template.
    Lang {
        key: LangKey,
        /// `<lang_or:key:'text':arg>`
        fallback: Option<Vec<StrSeg>>,
        args: Vec<Vec<Piece>>,
        style: Style,
        range: (usize, usize),
    },
    /// `<score:name:objective>` — a scoreboard value the server resolves.
    Score {
        name: String,
        objective: String,
        style: Style,
        range: (usize, usize),
    },
    /// `<selector:@a[:separator]>` — the entities a selector matches, the separator its own template.
    Selector {
        selector: String,
        separator: Option<Vec<Piece>>,
        style: Style,
        range: (usize, usize),
    },
    /// `<nbt:entity:'@s':Health[:separator][:interpret]>` — NBT data the server resolves.
    Nbt {
        source: NbtSourceIr,
        path: String,
        interpret: bool,
        separator: Option<Vec<Piece>>,
        style: Style,
        range: (usize, usize),
    },
    /// `<sprite[:atlas]:sprite>` — an atlas sprite drawn inline.
    Sprite {
        atlas: String,
        sprite: String,
        style: Style,
        range: (usize, usize),
    },
    /// `<head:name|uuid|texture[:outer_layer]>` — a player head drawn inline.
    Head {
        player: HeadIr,
        hat: bool,
        style: Style,
        range: (usize, usize),
    },
    Hole {
        arg: HoleArg,
        kind: HoleKind,
        /// Optional `format!` spec on a text hole (`{x:.2}` → `Some(".2")`).
        spec: Option<String>,
        style: Style,
        range: (usize, usize),
    },
}

impl Piece {
    /// Whether this piece is a hole or carries runtime styling.
    #[must_use]
    pub fn has_dyn(&self) -> bool {
        match self {
            Piece::Hole {
                kind: HoleKind::ConstText,
                style,
                ..
            } => style.has_dyn(),
            Piece::Hole { .. } => true,
            Piece::Lang {
                key,
                fallback,
                args,
                style,
                ..
            } => {
                matches!(key, LangKey::Dyn(_))
                    || fallback.as_deref().is_some_and(segs_have_dyn)
                    || args.iter().any(|arg| pieces_have_dyn(arg))
                    || style.has_dyn()
            }
            Piece::Selector {
                separator, style, ..
            }
            | Piece::Nbt {
                separator, style, ..
            } => separator.as_deref().is_some_and(pieces_have_dyn) || style.has_dyn(),
            other => other.style().has_dyn(),
        }
    }

    #[must_use]
    pub const fn style(&self) -> &Style {
        match self {
            Piece::Text { style, .. }
            | Piece::Keybind { style, .. }
            | Piece::Lang { style, .. }
            | Piece::Score { style, .. }
            | Piece::Selector { style, .. }
            | Piece::Nbt { style, .. }
            | Piece::Sprite { style, .. }
            | Piece::Head { style, .. }
            | Piece::Hole { style, .. } => style,
        }
    }

    #[must_use]
    pub(crate) const fn range(&self) -> (usize, usize) {
        match self {
            Piece::Text { range, .. }
            | Piece::Keybind { range, .. }
            | Piece::Lang { range, .. }
            | Piece::Score { range, .. }
            | Piece::Selector { range, .. }
            | Piece::Nbt { range, .. }
            | Piece::Sprite { range, .. }
            | Piece::Head { range, .. }
            | Piece::Hole { range, .. } => *range,
        }
    }

    pub(crate) const fn style_mut(&mut self) -> &mut Style {
        match self {
            Piece::Text { style, .. }
            | Piece::Keybind { style, .. }
            | Piece::Lang { style, .. }
            | Piece::Score { style, .. }
            | Piece::Selector { style, .. }
            | Piece::Nbt { style, .. }
            | Piece::Sprite { style, .. }
            | Piece::Head { style, .. }
            | Piece::Hole { style, .. } => style,
        }
    }

    /// How many gradient positions this piece occupies.
    pub(crate) fn width(&self) -> usize {
        match self {
            Piece::Text { text, .. } => text.chars().count(),
            _ => 1,
        }
    }

    /// Bare pieces (no styling, not components) encode as plain NBT strings.
    #[must_use]
    pub fn is_bare(&self) -> bool {
        match self {
            Piece::Hole {
                kind: HoleKind::Component | HoleKind::ConstComponent,
                ..
            }
            | Piece::Keybind { .. }
            | Piece::Lang { .. }
            | Piece::Score { .. }
            | Piece::Selector { .. }
            | Piece::Nbt { .. }
            | Piece::Sprite { .. }
            | Piece::Head { .. } => false,
            _ => *self.style() == Style::default(),
        }
    }
}

/// A parsed template.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub pieces: Vec<Piece>,
    /// How many positional holes the template contains.
    pub positional_count: usize,
}
