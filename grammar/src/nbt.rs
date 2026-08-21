//! Network-NBT emission over the grammar IR: a template becomes static byte
//! runs (`[tag type][payload]`) interleaved with runtime splice points ([`NbtSeg`]).

use std::{error::Error, fmt};

use crate::{
    BoolIr, ClickIr, ClickKind, ColorIr, HoleArg, HoleKind, HoverIr, LangKey, Piece, StrSeg, Style,
};

/// A template the NBT format cannot represent.
#[derive(Debug, Clone)]
pub struct EmitError {
    pub message: String,
    /// Char range of the piece the failure belongs to; [`None`] for whole-list limits.
    pub range: Option<(usize, usize)>,
}

impl EmitError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            range: None,
        }
    }
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for EmitError {}

/// Emits a full component tag stream (`[type byte][payload]`) for a template.
pub fn emit(pieces: &[Piece]) -> Result<Vec<NbtSeg>, EmitError> {
    let mut emitter = NbtEmitter {
        segs: Vec::new(),
        at: None,
    };
    emitter.component(pieces)?;
    Ok(emitter.segs)
}

pub(crate) const TAG_BYTE: u8 = 1;
pub(crate) const TAG_INT: u8 = 3;
pub(crate) const TAG_STRING: u8 = 8;
pub(crate) const TAG_LIST: u8 = 9;
pub(crate) const TAG_COMPOUND: u8 = 10;

#[derive(Debug, Clone, PartialEq)]
pub enum NbtSeg {
    Bytes(Vec<u8>),
    /// Dynamic string payload: u16 BE length + MUTF-8 bytes, written at runtime.
    Hole(HoleArg, Option<String>),
    /// Dynamic color name payload (`<{}>`): u16 length + name written at runtime.
    ColorHole(HoleArg),
    /// Dynamic formatting flag payload (`<b:{}>`): one byte at runtime.
    BoolHole(HoleArg),
    /// An NBT string assembled from literal runs and holes at runtime.
    StrSplice(Vec<StrSeg>),
    /// `{@}`: a component supplied at runtime, spliced in at the position it occupies.
    Component {
        arg: HoleArg,
        style: Box<Style>,
        at: SplicePos,
    },
    /// `{@const NAME}`: a `const` component the macro folds into the surrounding static bytes.
    ConstComponent {
        arg: HoleArg,
        at: SplicePos,
    },
    /// `<hover:{item}>` / `<click:{cmd}>`: the event's compound payload is encoded at runtime.
    EventHole {
        arg: HoleArg,
        kind: EventKind,
    },
    /// `<lang:{}:…>`: the runtime writes the u16 length + key of a `Translation<arity>`.
    LangKey {
        arg: HoleArg,
        arity: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    Hover,
    Click,
}

/// Where a spliced component lands.
#[derive(Debug, Clone, PartialEq)]
pub enum SplicePos {
    /// The entire message: the component's encoding, verbatim.
    Root,
    /// One element of a compound list (`extra`, `with`): the tag type byte belongs to the list.
    Elem,
    /// The value of a named compound entry, such as a hover's `value`.
    Entry(String),
}

struct ComponentHole<'p> {
    kind: HoleKind,
    arg: &'p HoleArg,
    style: &'p Style,
}

impl<'p> ComponentHole<'p> {
    const fn of(piece: &'p Piece) -> Option<Self> {
        match piece {
            Piece::Hole {
                kind: kind @ (HoleKind::Component | HoleKind::ConstComponent),
                arg,
                style,
                ..
            } => Some(Self {
                kind: *kind,
                arg,
                style,
            }),
            _ => None,
        }
    }
}

struct NbtEmitter {
    segs: Vec<NbtSeg>,
    /// Range of the piece currently being emitted, stamped onto errors raised inside it.
    at: Option<(usize, usize)>,
}

impl NbtEmitter {
    fn buf(&mut self) -> &mut Vec<u8> {
        if !matches!(self.segs.last(), Some(NbtSeg::Bytes(_))) {
            self.segs.push(NbtSeg::Bytes(Vec::new()));
        }
        match self
            .segs
            .last_mut()
            .expect("a trailing byte segment was just ensured")
        {
            NbtSeg::Bytes(b) => b,
            _ => unreachable!(),
        }
    }
    fn u8(&mut self, v: u8) {
        self.buf().push(v);
    }
    fn mutf8(&mut self, s: &str) -> Result<(), EmitError> {
        let b = to_mutf8(s);
        let len = u16::try_from(b.len()).map_err(|_| EmitError {
            message: format!(
                "text is {} bytes once encoded, over the 65535-byte limit an NBT string can \
                 carry — split it into several pieces",
                b.len()
            ),
            range: self.at,
        })?;
        let buf = self.buf();
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&b);
        Ok(())
    }
    fn entry(&mut self, tag: u8, name: &str) -> Result<(), EmitError> {
        self.u8(tag);
        self.mutf8(name)
    }
    fn string_entry(&mut self, name: &str, value: &str) -> Result<(), EmitError> {
        self.entry(TAG_STRING, name)?;
        self.mutf8(value)
    }

    /// Mirrors `NbtBuilder::build_component`'s field order exactly.
    fn component(&mut self, pieces: &[Piece]) -> Result<(), EmitError> {
        if let [piece] = pieces
            && let Some(hole) = ComponentHole::of(piece)
        {
            return self.splice(hole, SplicePos::Root);
        }
        self.u8(component_tag_type(pieces));
        self.component_payload(pieces)
    }

    fn splice(&mut self, hole: ComponentHole<'_>, at: SplicePos) -> Result<(), EmitError> {
        let ComponentHole { kind, arg, style } = hole;
        if kind != HoleKind::ConstComponent {
            self.segs.push(NbtSeg::Component {
                arg: arg.clone(),
                style: Box::new(style.clone()),
                at,
            });
            return Ok(());
        }
        if *style == Style::default() {
            self.segs.push(NbtSeg::ConstComponent {
                arg: arg.clone(),
                at,
            });
            return Ok(());
        }
        // NBT's own style inheritance applies the tags to the spliced bytes
        match &at {
            SplicePos::Root => self.u8(TAG_COMPOUND),
            SplicePos::Elem => {}
            SplicePos::Entry(name) => self.entry(TAG_COMPOUND, name)?,
        }
        self.string_entry("text", "")?;
        self.style_entries(style)?;
        self.entry(TAG_LIST, "extra")?;
        self.u8(TAG_COMPOUND);
        self.buf().extend_from_slice(&1i32.to_be_bytes());
        self.segs.push(NbtSeg::ConstComponent {
            arg: arg.clone(),
            at: SplicePos::Elem,
        });
        self.u8(0);
        Ok(())
    }

    fn bare_payload(&mut self, piece: &Piece) -> Result<(), EmitError> {
        self.at = Some(piece.range());
        match piece {
            Piece::Text { text, .. } => self.mutf8(text)?,
            Piece::Hole { arg, spec, .. } => {
                self.segs.push(NbtSeg::Hole(arg.clone(), spec.clone()));
            }
            Piece::Keybind { .. } | Piece::Lang { .. } => {
                unreachable!("keybinds and translations are never bare")
            }
        }
        Ok(())
    }

    fn str_value(&mut self, name: &str, segs: &[StrSeg]) -> Result<(), EmitError> {
        if let Some(value) = StrSeg::join_lits(segs) {
            self.string_entry(name, &value)?;
        } else {
            self.entry(TAG_STRING, name)?;
            self.segs.push(NbtSeg::StrSplice(segs.to_vec()));
        }
        Ok(())
    }

    fn component_payload(&mut self, pieces: &[Piece]) -> Result<(), EmitError> {
        // a present `extra` may not be empty, so an empty template is the empty string
        if pieces.is_empty() {
            return self.mutf8("");
        }
        if let [piece] = pieces {
            if piece.is_bare() {
                self.bare_payload(piece)?;
            } else {
                self.leaf_compound_payload(piece)?;
            }
            return Ok(());
        }
        self.string_entry("text", "")?;
        self.entry(TAG_LIST, "extra")?;
        let all_bare = pieces.iter().all(Piece::is_bare);
        let elem = if all_bare { TAG_STRING } else { TAG_COMPOUND };
        self.u8(elem);
        let count = i32::try_from(pieces.len()).map_err(|_| {
            EmitError::new(format!(
                "{} pieces in one component, more than the i32 an NBT list length is",
                pieces.len()
            ))
        })?;
        self.buf().extend_from_slice(&count.to_be_bytes());
        for piece in pieces {
            if let Some(hole) = ComponentHole::of(piece) {
                self.splice(hole, SplicePos::Elem)?;
            } else if all_bare {
                self.bare_payload(piece)?;
            } else if piece.is_bare() {
                // vanilla's heterogeneous-list convention: a bare string is wrapped as {"": value}
                self.entry(TAG_STRING, "")?;
                self.bare_payload(piece)?;
                self.u8(0);
            } else {
                self.leaf_compound_payload(piece)?;
            }
        }
        self.u8(0);
        Ok(())
    }

    fn lang_content(&mut self, key: &LangKey, args: &[Vec<Piece>]) -> Result<(), EmitError> {
        self.entry(TAG_STRING, "translate")?;
        match key {
            LangKey::Lit(k) => self.mutf8(k)?,
            LangKey::Dyn(arg) => self.segs.push(NbtSeg::LangKey {
                arg: arg.clone(),
                arity: args.len(),
            }),
        }
        if args.is_empty() {
            return Ok(());
        }
        self.entry(TAG_LIST, "with")?;
        // an empty arg encodes as the empty string, so it keeps the list bare
        let all_bare = args.iter().all(|a| match &a[..] {
            [] => true,
            [piece] => piece.is_bare(),
            _ => false,
        });
        self.u8(if all_bare { TAG_STRING } else { TAG_COMPOUND });
        let count = i32::try_from(args.len()).map_err(|_| {
            EmitError::new(format!(
                "{} translation args, more than the i32 an NBT list length is",
                args.len()
            ))
        })?;
        self.buf().extend_from_slice(&count.to_be_bytes());
        for arg in args {
            if all_bare {
                self.component_payload(arg)?;
            } else {
                self.with_elem(arg)?;
            }
        }
        Ok(())
    }

    fn with_elem(&mut self, pieces: &[Piece]) -> Result<(), EmitError> {
        match pieces {
            [] => {
                self.entry(TAG_STRING, "")?;
                self.mutf8("")?;
                self.u8(0);
            }
            [piece] if let Some(hole) = ComponentHole::of(piece) => {
                self.splice(hole, SplicePos::Elem)?;
            }
            [piece] if piece.is_bare() => {
                self.entry(TAG_STRING, "")?;
                self.bare_payload(piece)?;
                self.u8(0);
            }
            [piece] => self.leaf_compound_payload(piece)?,
            _ => self.component_payload(pieces)?,
        }
        Ok(())
    }

    /// Matches `to_compound`'s field order.
    fn leaf_compound_payload(&mut self, piece: &Piece) -> Result<(), EmitError> {
        self.at = Some(piece.range());
        match piece {
            Piece::Keybind { key, .. } => self.string_entry("keybind", key)?,
            Piece::Lang { key, args, .. } => self.lang_content(key, args)?,
            _ => {
                self.entry(TAG_STRING, "text")?;
                self.bare_payload(piece)?;
            }
        }
        self.style_entries(piece.style())?;
        self.u8(0);
        Ok(())
    }

    /// The styling entries of a compound, in `to_compound` field order.
    fn style_entries(&mut self, style: &Style) -> Result<(), EmitError> {
        let at = self.at;
        if let Some(color) = &style.color {
            match color {
                ColorIr::Named(n) => self.string_entry("color", n)?,
                ColorIr::Rgb(r, g, b) => {
                    self.string_entry("color", &format!("#{r:02X}{g:02X}{b:02X}"))?;
                }
                ColorIr::Dyn(arg) => {
                    self.entry(TAG_STRING, "color")?;
                    self.segs.push(NbtSeg::ColorHole(arg.clone()));
                }
            }
        }
        if let Some(font) = &style.font {
            self.string_entry("font", font)?;
        }
        for (name, v) in [
            ("bold", &style.bold),
            ("italic", &style.italic),
            ("underlined", &style.underlined),
            ("strikethrough", &style.strikethrough),
            ("obfuscated", &style.obfuscated),
        ] {
            if let Some(v) = v {
                self.entry(TAG_BYTE, name)?;
                match v {
                    BoolIr::Const(b) => self.u8(u8::from(*b)),
                    BoolIr::Dyn(arg) => self.segs.push(NbtSeg::BoolHole(arg.clone())),
                }
            }
        }
        if let Some(shadow) = style.shadow_color {
            self.entry(TAG_INT, "shadow_color")?;
            self.buf().extend_from_slice(&shadow.to_be_bytes());
        }
        if let Some(segs) = &style.insertion {
            self.str_value("insertion", segs)?;
        }
        match &style.hover {
            None => {}
            Some(HoverIr::Dyn(arg)) => {
                self.entry(TAG_COMPOUND, "hover_event")?;
                self.segs.push(NbtSeg::EventHole {
                    arg: arg.clone(),
                    kind: EventKind::Hover,
                });
            }
            Some(HoverIr::Text(hover)) => {
                self.entry(TAG_COMPOUND, "hover_event")?;
                self.string_entry("action", "show_text")?;
                // a spliced component decides its own tag type, so its entry header waits
                if let [piece] = &hover[..]
                    && let Some(hole) = ComponentHole::of(piece)
                {
                    self.splice(hole, SplicePos::Entry("value".to_string()))?;
                } else {
                    self.entry(component_tag_type(hover), "value")?;
                    self.component_payload(hover)?;
                }
                self.u8(0);
                // the hover's own pieces moved the cursor off this piece
                self.at = at;
            }
        }
        match &style.click {
            None => {}
            Some(ClickIr::Dyn(arg)) => {
                self.entry(TAG_COMPOUND, "click_event")?;
                self.segs.push(NbtSeg::EventHole {
                    arg: arg.clone(),
                    kind: EventKind::Click,
                });
            }
            Some(ClickIr::Action(kind, segs)) => {
                self.entry(TAG_COMPOUND, "click_event")?;
                let (action, field) = match kind {
                    ClickKind::OpenUrl => ("open_url", "url"),
                    ClickKind::RunCommand => ("run_command", "command"),
                    ClickKind::SuggestCommand => ("suggest_command", "command"),
                    ClickKind::CopyToClipboard => ("copy_to_clipboard", "value"),
                    ClickKind::ChangePage(_) => ("change_page", "page"),
                };
                self.string_entry("action", action)?;
                if let ClickKind::ChangePage(page) = kind {
                    self.entry(TAG_INT, field)?;
                    self.buf().extend_from_slice(&page.to_be_bytes());
                } else {
                    self.str_value(field, segs)?;
                }
                self.u8(0);
            }
        }
        Ok(())
    }
}

#[must_use]
pub(crate) fn component_tag_type(pieces: &[Piece]) -> u8 {
    match pieces {
        [] => TAG_STRING,
        [piece] if piece.is_bare() => TAG_STRING,
        _ => TAG_COMPOUND,
    }
}

/// Java Modified UTF-8: NUL → C0 80, BMP as UTF-8, supplementary as CESU-8.
#[must_use]
pub fn to_mutf8(s: &str) -> Vec<u8> {
    fn push3(out: &mut Vec<u8>, c: u32) {
        out.push(0xE0 | (c >> 12) as u8);
        out.push(0x80 | ((c >> 6) & 0x3F) as u8);
        out.push(0x80 | (c & 0x3F) as u8);
    }
    let mut out = Vec::with_capacity(s.len());
    for ch in s.chars() {
        let c = ch as u32;
        match c {
            0 => out.extend_from_slice(&[0xC0, 0x80]),
            0x01..=0x7F => out.push(c as u8),
            0x80..=0x7FF => {
                out.push(0xC0 | (c >> 6) as u8);
                out.push(0x80 | (c & 0x3F) as u8);
            }
            0x800..=0xFFFF => push3(&mut out, c),
            _ => {
                let v = c - 0x1_0000;
                push3(&mut out, 0xD800 + (v >> 10));
                push3(&mut out, 0xDC00 + (v & 0x3FF));
            }
        }
    }
    out
}
