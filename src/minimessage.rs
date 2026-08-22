//! Parse a `MiniMessage` template at run time, from a config file or another
//! data source, then fill the placeholders. The tags are the same ones `text!` uses.

#[cfg(feature = "nbt")]
use crate::{
    EncodedComponent,
    nbt::{
        write_color, write_component_elem, write_component_entry, write_component_root,
        write_mutf8_body,
    },
};
use crate::{
    TextComponent,
    content::Content,
    format::Color,
    interactivity::{ClickEvent, HoverEvent, MaybeStatic},
    translation::TranslatedMessage,
};
use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;
#[cfg(feature = "nbt")]
use text_components_grammar::nbt::{EventKind, NbtSeg, SplicePos, emit};
use text_components_grammar::{
    BoolIr, ClickIr, ClickKind, ColorIr, HoleArg, HoleKind, HoverIr, LangKey, Mode, ParseError,
    Piece, StrSeg, Style, parse,
};

/// The value of one hole, passed to [`MiniMessage::fill`].
#[derive(Debug, Clone)]
pub enum Value {
    /// Fills `{name}`. Inserted as plain text, never parsed for tags.
    Text(String),
    /// Fills `{@name}`. Keeps its own styling.
    Component(TextComponent),
    /// Fills `{@name}` in [`MiniMessage::fill_nbt`]. The bytes are copied in
    /// without decoding.
    #[cfg(feature = "nbt")]
    Encoded(EncodedComponent),
    /// Fills `<{name}>`.
    Color(Color),
    /// Fills `<b:{name}>` and the other formatting flags.
    Flag(bool),
    /// Fills `<hover:{name}>`.
    Hover(HoverEvent),
    /// Fills `<click:{name}>`.
    Click(ClickEvent),
}

impl Value {
    const fn kind_name(&self) -> &'static str {
        match self {
            Value::Text(_) => "text",
            Value::Component(_) => "a component",
            #[cfg(feature = "nbt")]
            Value::Encoded(_) => "an encoded component",
            Value::Color(_) => "a color",
            Value::Flag(_) => "a flag",
            Value::Hover(_) => "a hover event",
            Value::Click(_) => "a click event",
        }
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Text(v.to_string())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}
impl From<TextComponent> for Value {
    fn from(v: TextComponent) -> Self {
        Self::Component(v)
    }
}
#[cfg(feature = "nbt")]
impl From<EncodedComponent> for Value {
    fn from(v: EncodedComponent) -> Self {
        Self::Encoded(v)
    }
}
impl From<Color> for Value {
    fn from(v: Color) -> Self {
        Self::Color(v)
    }
}
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Flag(v)
    }
}
impl From<HoverEvent> for Value {
    fn from(v: HoverEvent) -> Self {
        Self::Hover(v)
    }
}
impl From<ClickEvent> for Value {
    fn from(v: ClickEvent) -> Self {
        Self::Click(v)
    }
}

/// What an [`Error`] is about.
#[derive(Debug, Clone)]
pub enum ErrorKind {
    /// The template does not parse.
    Parse {
        error: ParseError,
        template: Box<str>,
    },
    /// A value was missing or had the wrong type.
    Fill(String),
}

/// A parse or fill error. `Display` renders the message, and for a parse error
/// also the position in the template and the help line.
#[derive(Debug, Clone)]
pub struct Error {
    kind: ErrorKind,
}

impl Error {
    fn fill(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Fill(msg.into()),
        }
    }

    /// What this error is about.
    #[must_use]
    pub const fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// The character range in the template a parse error points at.
    #[must_use]
    pub const fn range(&self) -> Option<(usize, usize)> {
        match &self.kind {
            ErrorKind::Parse { error, .. } => Some(error.range),
            ErrorKind::Fill(_) => None,
        }
    }

    /// The help line attached to a parse error, if any.
    #[must_use]
    pub const fn help(&self) -> Option<&str> {
        match &self.kind {
            ErrorKind::Parse { error, .. } => match &error.help {
                Some(help) => Some(help.as_str()),
                None => None,
            },
            ErrorKind::Fill(_) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Parse { error, template } => f.write_str(&error.render(template)),
            ErrorKind::Fill(msg) => f.write_str(msg),
        }
    }
}
impl StdError for Error {}

fn hole_name(arg: &HoleArg) -> &str {
    match arg {
        HoleArg::Named { name, .. } => name,
        HoleArg::Positional(_) => unreachable!("rejected at parse time"),
    }
}

fn lookup<'v>(values: &'v [(&str, Value)], name: &str) -> Result<&'v Value, Error> {
    values
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v)
        .ok_or_else(|| {
            let available: Vec<&str> = values.iter().map(|(n, _)| *n).collect();
            Error::fill(format!(
                "template needs a value for `{{{name}}}`, provided: [{}]",
                available.join(", ")
            ))
        })
}

/// Joins literal runs and filled holes into one string value.
fn resolve_string(segs: &[StrSeg], what: &str, values: &[(&str, Value)]) -> Result<String, Error> {
    let mut value = String::new();
    for seg in segs {
        match seg {
            StrSeg::Lit(lit) => value.push_str(lit),
            StrSeg::Hole(arg, _) => {
                let name = hole_name(arg);
                match lookup(values, name)? {
                    Value::Text(s) => value.push_str(s),
                    other => {
                        return Err(Error::fill(format!(
                            "{what} hole `{{{name}}}` needs a text value, got {}",
                            other.kind_name()
                        )));
                    }
                }
            }
        }
    }
    Ok(value)
}

#[cfg(feature = "nbt")]
fn patch_str_len(buf: &mut [u8], at: usize) -> Result<(), Error> {
    let n = u16::try_from(buf.len() - at - 2)
        .map_err(|_| Error::fill("filled value exceeds the 65535-byte limit of an NBT string"))?;
    buf[at..at + 2].copy_from_slice(&n.to_be_bytes());
    Ok(())
}

/// A parsed `MiniMessage` template. Parse once, fill as often as needed.
///
/// ```
/// # use text_components::minimessage::{MiniMessage, Value};
/// let template = MiniMessage::new("<yellow>{player}</yellow><gray> joined</gray>")?;
/// let msg = template.fill(&[("player", Value::from("Notch"))])?;
/// # Ok::<(), text_components::minimessage::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MiniMessage {
    pieces: Vec<Piece>,
    #[cfg(feature = "nbt")]
    segments: Vec<NbtSeg>,
}

impl MiniMessage {
    /// Parses a template. The error carries the position in the template and a
    /// help line, both rendered by `Display`.
    pub fn new(input: &str) -> Result<Self, Error> {
        let template = parse(input, Mode::Runtime).map_err(|error| Error {
            kind: ErrorKind::Parse {
                error,
                template: input.into(),
            },
        })?;
        #[cfg(feature = "nbt")]
        let segments = emit(&template.pieces).map_err(|e| match e.range {
            Some(range) => Error {
                kind: ErrorKind::Parse {
                    error: ParseError {
                        range,
                        message: e.message,
                        help: None,
                    },
                    template: input.into(),
                },
            },
            None => Error::fill(e.message),
        })?;
        Ok(Self {
            pieces: template.pieces,
            #[cfg(feature = "nbt")]
            segments,
        })
    }

    /// The hole names this template needs, sorted and deduplicated.
    pub fn holes(&self) -> impl Iterator<Item = &str> {
        fn collect<'p>(pieces: &'p [Piece], out: &mut Vec<&'p str>) {
            for piece in pieces {
                if let Piece::Hole { arg, .. } = piece {
                    out.push(hole_name(arg));
                }
                if let Piece::Lang {
                    key,
                    fallback,
                    args,
                    ..
                } = piece
                {
                    if let LangKey::Dyn(arg) = key {
                        out.push(hole_name(arg));
                    }
                    if let Some(segs) = fallback {
                        for seg in segs {
                            if let StrSeg::Hole(arg, _) = seg {
                                out.push(hole_name(arg));
                            }
                        }
                    }
                    for arg in args {
                        collect(arg, out);
                    }
                }
                let style = piece.style();
                if let Some(ColorIr::Dyn(arg)) = &style.color {
                    out.push(hole_name(arg));
                }
                for (_, flag) in style.flags() {
                    if let Some(BoolIr::Dyn(arg)) = flag {
                        out.push(hole_name(arg));
                    }
                }
                let collect_segs = |segs: &'p [StrSeg], out: &mut Vec<&'p str>| {
                    for seg in segs {
                        if let StrSeg::Hole(arg, _) = seg {
                            out.push(hole_name(arg));
                        }
                    }
                };
                if let Some(segs) = &style.insertion {
                    collect_segs(segs, out);
                }
                match &style.hover {
                    Some(HoverIr::Text(hover)) => collect(hover, out),
                    Some(HoverIr::Dyn(arg)) => out.push(hole_name(arg)),
                    None => {}
                }
                match &style.click {
                    Some(ClickIr::Action(_, segs)) => collect_segs(segs, out),
                    Some(ClickIr::Dyn(arg)) => out.push(hole_name(arg)),
                    None => {}
                }
            }
        }
        let mut names = Vec::new();
        collect(&self.pieces, &mut names);
        names.sort_unstable();
        names.dedup();
        names.into_iter()
    }

    /// Fills the template. A missing value, or one of the wrong type, is an
    /// error.
    pub fn fill(&self, values: &[(&str, Value)]) -> Result<TextComponent, Error> {
        Self::fill_pieces(&self.pieces, values)
    }

    fn fill_pieces(pieces: &[Piece], values: &[(&str, Value)]) -> Result<TextComponent, Error> {
        let mut children = Vec::with_capacity(pieces.len());
        for piece in pieces {
            children.push(Self::build_piece(piece, values)?);
        }
        Ok(if children.len() == 1 {
            children
                .into_iter()
                .next()
                .expect("length was just checked to be one")
        } else {
            let mut root = TextComponent::plain("");
            root.children = Cow::Owned(children);
            root
        })
    }

    /// Fills the template and returns the message encoded as network NBT.
    #[cfg(feature = "nbt")]
    #[expect(
        clippy::too_many_lines,
        reason = "one match arm per tag/variant; splitting hides the shape"
    )]
    pub fn fill_nbt(&self, values: &[(&str, Value)]) -> Result<EncodedComponent, Error> {
        let static_len: usize = self
            .segments
            .iter()
            .map(|s| match s {
                NbtSeg::Bytes(b) => b.len(),
                _ => 16,
            })
            .sum();
        let mut buf = Vec::with_capacity(static_len + 64);
        for segment in &self.segments {
            match segment {
                NbtSeg::Bytes(bytes) => buf.extend_from_slice(bytes),
                NbtSeg::Hole(arg, _) => {
                    let name = hole_name(arg);
                    match lookup(values, name)? {
                        Value::Text(s) => {
                            let at = buf.len();
                            buf.extend_from_slice(&[0, 0]);
                            write_mutf8_body(&mut buf, s);
                            patch_str_len(&mut buf, at)?;
                        }
                        other => {
                            return Err(Error::fill(format!(
                                "`{{{name}}}` needs a text value, got {}",
                                other.kind_name()
                            )));
                        }
                    }
                }
                NbtSeg::LangKey { arg, .. } => {
                    let name = hole_name(arg);
                    match lookup(values, name)? {
                        Value::Text(key) => {
                            let at = buf.len();
                            buf.extend_from_slice(&[0, 0]);
                            write_mutf8_body(&mut buf, key);
                            patch_str_len(&mut buf, at)?;
                        }
                        other => {
                            return Err(Error::fill(format!(
                                "`<lang:{{{name}}}>` needs a text key, got {}",
                                other.kind_name()
                            )));
                        }
                    }
                }
                NbtSeg::ColorHole(arg) => {
                    let name = hole_name(arg);
                    match lookup(values, name)? {
                        Value::Color(color) => write_color(&mut buf, color),
                        other => {
                            return Err(Error::fill(format!(
                                "`<{{{name}}}>` needs a Color value, got {}",
                                other.kind_name()
                            )));
                        }
                    }
                }
                NbtSeg::StrSplice(parts) => {
                    let at = buf.len();
                    buf.extend_from_slice(&[0, 0]);
                    for part in parts {
                        match part {
                            StrSeg::Lit(lit) => write_mutf8_body(&mut buf, lit),
                            StrSeg::Hole(arg, _) => {
                                let name = hole_name(arg);
                                match lookup(values, name)? {
                                    Value::Text(s) => write_mutf8_body(&mut buf, s),
                                    other => {
                                        return Err(Error::fill(format!(
                                            "click hole `{{{name}}}` needs a text value, got {}",
                                            other.kind_name()
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    patch_str_len(&mut buf, at)?;
                }
                NbtSeg::BoolHole(arg) => {
                    let name = hole_name(arg);
                    match lookup(values, name)? {
                        Value::Flag(v) => buf.push(u8::from(*v)),
                        other => {
                            return Err(Error::fill(format!(
                                "flag hole `{{{name}}}` needs a bool value, got {}",
                                other.kind_name()
                            )));
                        }
                    }
                }
                NbtSeg::EventHole { arg, kind } => {
                    let name = hole_name(arg);
                    let tag = match (kind, lookup(values, name)?) {
                        (EventKind::Hover, Value::Hover(event)) => event.to_codec_nbt(),
                        (EventKind::Click, Value::Click(event)) => event.to_codec_nbt(),
                        (EventKind::Hover, other) => {
                            return Err(Error::fill(format!(
                                "`<hover:{{{name}}}>` needs a HoverEvent value, got {}",
                                other.kind_name()
                            )));
                        }
                        (EventKind::Click, other) => {
                            return Err(Error::fill(format!(
                                "`<click:{{{name}}}>` needs a ClickEvent value, got {}",
                                other.kind_name()
                            )));
                        }
                    };
                    let mut tmp = Vec::with_capacity(128);
                    tag.write(&mut tmp);
                    buf.extend_from_slice(&tmp[1..]);
                }
                NbtSeg::Component { arg, style, at } => {
                    let name = hole_name(arg);
                    match lookup(values, name)? {
                        Value::Component(component) => {
                            let styled = Self::apply_style(component.clone(), style, values, true)?;
                            match at {
                                SplicePos::Root => write_component_root(&mut buf, &styled),
                                SplicePos::Elem => write_component_elem(&mut buf, &styled),
                                SplicePos::Entry(entry) => {
                                    write_component_entry(&mut buf, entry, &styled);
                                }
                            }
                        }
                        Value::Encoded(encoded) => {
                            let carrier = if **style == Style::default() {
                                None
                            } else {
                                Some(Self::apply_style(
                                    TextComponent::plain(""),
                                    style,
                                    values,
                                    false,
                                )?)
                            };
                            let encoded = encoded.clone();
                            match at {
                                SplicePos::Root => encoded.splice_root(&mut buf, carrier.as_ref()),
                                SplicePos::Elem => encoded.splice_elem(&mut buf, carrier.as_ref()),
                                SplicePos::Entry(entry) => {
                                    encoded.splice_entry(&mut buf, entry, carrier.as_ref());
                                }
                            }
                        }
                        other => {
                            return Err(Error::fill(format!(
                                "`{{@{name}}}` needs a component value, got {}",
                                other.kind_name()
                            )));
                        }
                    }
                }
                NbtSeg::ConstComponent { .. } => {
                    unreachable!("const splices are rejected in runtime templates")
                }
            }
        }
        Ok(EncodedComponent::from_vec(buf))
    }

    fn build_piece(piece: &Piece, values: &[(&str, Value)]) -> Result<TextComponent, Error> {
        match piece {
            Piece::Text { text, style, .. } => {
                let component = TextComponent::plain(text.clone());
                Self::apply_style(component, style, values, false)
            }
            Piece::Keybind { key, style, .. } => {
                let component = TextComponent::from(Content::Keybind {
                    keybind: Cow::Owned(key.clone()),
                });
                Self::apply_style(component, style, values, false)
            }
            Piece::Lang {
                key,
                fallback,
                args,
                style,
                ..
            } => {
                let key = match key {
                    LangKey::Lit(key) => key.clone(),
                    LangKey::Dyn(arg) => {
                        let name = hole_name(arg);
                        match lookup(values, name)? {
                            Value::Text(key) => key.clone(),
                            other => {
                                return Err(Error::fill(format!(
                                    "`<lang:{{{name}}}>` needs a text key, got {}",
                                    other.kind_name()
                                )));
                            }
                        }
                    }
                };
                let fallback = match fallback {
                    Some(segs) => Some(Cow::Owned(resolve_string(segs, "fallback", values)?)),
                    None => None,
                };
                let mut translated = Vec::with_capacity(args.len());
                for arg in args {
                    translated.push(Self::fill_pieces(arg, values)?);
                }
                let component = TextComponent::translated(TranslatedMessage {
                    key: Cow::Owned(key),
                    fallback,
                    args: if translated.is_empty() {
                        crate::Args::None
                    } else {
                        crate::Args::Owned(translated.into_boxed_slice())
                    },
                });
                Self::apply_style(component, style, values, false)
            }
            Piece::Hole {
                arg, kind, style, ..
            } => {
                let name = hole_name(arg);
                let component = match (kind, lookup(values, name)?) {
                    (HoleKind::Text, Value::Text(s)) => TextComponent::plain(s.clone()),
                    (HoleKind::Component, Value::Component(c)) => c.clone(),
                    (HoleKind::Text, Value::Component(_)) => {
                        return Err(Error::fill(format!(
                            "`{{{name}}}` is a text hole but the value is a component — \
                             write {{@{name}}}"
                        )));
                    }
                    (HoleKind::Component, Value::Text(_)) => {
                        return Err(Error::fill(format!(
                            "`{{@{name}}}` is a component hole but the value is text — \
                             write {{{name}}}"
                        )));
                    }
                    #[cfg(feature = "nbt")]
                    (HoleKind::Component, Value::Encoded(_)) => {
                        return Err(Error::fill(format!(
                            "`{{@{name}}}` got an EncodedComponent. fill() cannot use it; \
                             use fill_nbt(), or pass a TextComponent"
                        )));
                    }
                    (HoleKind::ConstComponent, _) => {
                        unreachable!("const splices are rejected in runtime templates")
                    }
                    (_, other) => {
                        return Err(Error::fill(format!(
                            "`{{{name}}}` can't take {}",
                            other.kind_name()
                        )));
                    }
                };
                Self::apply_style(component, style, values, true)
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match arm per tag/variant; splitting hides the shape"
    )]
    fn apply_style(
        mut component: TextComponent,
        style: &Style,
        values: &[(&str, Value)],
        fill_if_unset: bool,
    ) -> Result<TextComponent, Error> {
        let resolve_color = |c: &ColorIr| -> Result<Color, Error> {
            match c {
                ColorIr::Named(name) => Color::from_codec_name(name).ok_or_else(|| {
                    Error::fill(format!(
                        "the grammar produced a color `{name}` this Color version doesn't know"
                    ))
                }),
                ColorIr::Rgb(r, g, b) => Ok(Color::Rgb(*r, *g, *b)),
                ColorIr::Dyn(HoleArg::Named { name, .. }) => match lookup(values, name)? {
                    Value::Color(color) => Ok(color.clone()),
                    other => Err(Error::fill(format!(
                        "`<{{{name}}}>` needs a Color value, got {}",
                        other.kind_name()
                    ))),
                },
                ColorIr::Dyn(HoleArg::Positional(_)) => unreachable!("rejected at parse time"),
            }
        };
        let resolve_flag = |b: &BoolIr| -> Result<bool, Error> {
            match b {
                BoolIr::Const(v) => Ok(*v),
                BoolIr::Dyn(HoleArg::Named { name, .. }) => match lookup(values, name)? {
                    Value::Flag(v) => Ok(*v),
                    other => Err(Error::fill(format!(
                        "flag hole `{{{name}}}` needs a bool value, got {}",
                        other.kind_name()
                    ))),
                },
                BoolIr::Dyn(HoleArg::Positional(_)) => unreachable!("rejected at parse time"),
            }
        };

        if let Some(color) = &style.color {
            let color = resolve_color(color)?;
            if !fill_if_unset || component.format.color.is_none() {
                component.format.color = Some(color);
            }
        }
        macro_rules! flag {
            ($f:ident) => {
                if let Some(v) = &style.$f {
                    let v = resolve_flag(v)?;
                    if !fill_if_unset || component.format.$f.is_none() {
                        component.format.$f = Some(v);
                    }
                }
            };
        }
        flag!(bold);
        flag!(italic);
        flag!(underlined);
        flag!(strikethrough);
        flag!(obfuscated);

        if let Some(font) = &style.font
            && (!fill_if_unset || component.format.font.is_none())
        {
            component.format.font = Some(Cow::Owned(font.clone()));
        }
        if let Some(shadow) = style.shadow_color
            && (!fill_if_unset || component.format.shadow_color.is_none())
        {
            component.format.shadow_color = Some(shadow);
        }

        if let Some(segs) = &style.insertion {
            let value = resolve_string(segs, "insertion", values)?;
            if !fill_if_unset || component.interactions.insertion.is_none() {
                component.interactions.insertion = Some(Cow::Owned(value));
            }
        }
        if let Some(hover) = &style.hover {
            let event = match hover {
                HoverIr::Dyn(arg) => {
                    let name = hole_name(arg);
                    match lookup(values, name)? {
                        Value::Hover(event) => event.clone(),
                        other => {
                            return Err(Error::fill(format!(
                                "`<hover:{{{name}}}>` needs a HoverEvent value, got {}",
                                other.kind_name()
                            )));
                        }
                    }
                }
                HoverIr::Text(hover_pieces) => HoverEvent::ShowText {
                    value: MaybeStatic::Owned(Box::new(Self::fill_pieces(hover_pieces, values)?)),
                },
            };
            if !fill_if_unset || component.interactions.hover.is_none() {
                component.interactions.hover = Some(MaybeStatic::Owned(Box::new(event)));
            }
        }
        if let Some(click) = &style.click {
            let event = match click {
                ClickIr::Dyn(arg) => {
                    let name = hole_name(arg);
                    match lookup(values, name)? {
                        Value::Click(event) => event.clone(),
                        other => {
                            return Err(Error::fill(format!(
                                "`<click:{{{name}}}>` needs a ClickEvent value, got {}",
                                other.kind_name()
                            )));
                        }
                    }
                }
                ClickIr::Action(kind, segs) => {
                    let value = resolve_string(segs, "click", values)?;
                    match kind {
                        ClickKind::OpenUrl => ClickEvent::open_url(value),
                        ClickKind::RunCommand => ClickEvent::run_command(value),
                        ClickKind::SuggestCommand => ClickEvent::suggest_command(value),
                        ClickKind::CopyToClipboard => ClickEvent::CopyToClipboard {
                            value: Cow::Owned(value),
                        },
                        ClickKind::ChangePage(page) => ClickEvent::ChangePage { page: *page },
                        ClickKind::ShowDialog => ClickEvent::show_dialog(value),
                    }
                }
            };
            if !fill_if_unset || component.interactions.click.is_none() {
                component.interactions.click = Some(MaybeStatic::Owned(Box::new(event)));
            }
        }
        Ok(component)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grammar's color table and [`Color`] are edited independently.
    #[test]
    fn canonical_color_names_round_trip() {
        for (_, canonical, _) in text_components_grammar::NAMED_COLORS {
            let color = Color::from_codec_name(canonical)
                .unwrap_or_else(|| panic!("`Color` has no variant for `{canonical}`"));
            assert_eq!(color.codec_name(), *canonical);
        }
    }
}
