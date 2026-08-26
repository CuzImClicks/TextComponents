use std::mem;

use crate::error::suggest;
use crate::tags::{COLOR_HELP, NAMED_COLORS, SUPPORTED_SUMMARY, all_tag_names, canonical_tag};
use crate::{
    BoolIr, ClickIr, ClickKind, ColorIr, HeadIr, HoleArg, HoleKind, HoverIr, LangKey, NbtSourceIr,
    ParseError, Piece, StrSeg, Style, Template,
};

/// Where a template comes from, which decides what its holes may look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// text! / `text_nbt!`: positional holes and format specs allowed.
    Macro,
    /// Config-file templates: named holes only, no specs.
    Runtime,
}

impl Mode {
    fn allow_positional(self) -> bool {
        self == Mode::Macro
    }

    fn allow_specs(self) -> bool {
        self == Mode::Macro
    }

    fn allow_const_holes(self) -> bool {
        self == Mode::Macro
    }
}

/// Parses a `MiniMessage` template into its pieces.
pub fn parse(input: &str, mode: Mode) -> Result<Template, ParseError> {
    let mut positional = 0usize;
    let pieces = Parser {
        chars: input.chars().collect(),
        pos: 0,
        mode,
        positional: &mut positional,
        offset: 0,
    }
    .parse()?;
    Ok(Template {
        pieces,
        positional_count: positional,
    })
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    mode: Mode,
    positional: &'a mut usize,
    /// Offset inside the outermost template, so nested parsers report the right range.
    offset: usize,
}

/// Which argument fills a hole, what it takes, its optional `format!` spec, and its range.
type ParsedHole = (HoleArg, HoleKind, Option<String>, (usize, usize));

/// One `<…>` tag as read from the template.
struct ParsedTag {
    parts: Vec<TagArg>,
    range: (usize, usize),
    self_closing: bool,
}

struct TagArg {
    text: String,
    range: (usize, usize),
}

enum ArgState {
    Fresh,
    Unquoted,
    Quoted {
        quote: char,
        open: usize,
        content_start: usize,
    },
    Closed {
        quote: char,
        range: (usize, usize),
    },
}

struct GradMark {
    stack_idx: usize,
    piece_start: usize,
    /// Empty for `<rainbow>`.
    stops: Vec<(u8, u8, u8)>,
}

#[derive(Default)]
struct Ctx {
    pieces: Vec<Piece>,
    /// (canonical tag name, style snapshot from before the tag opened)
    stack: Vec<(String, Style)>,
    style: Style,
    text: String,
    text_range: (usize, usize),
    grads: Vec<GradMark>,
}

impl Ctx {
    /// `at` is the source span the character costs: `\\<` spans two, `<br>` the whole tag.
    fn push_text(&mut self, ch: char, at: (usize, usize)) {
        if self.text.is_empty() {
            self.text_range.0 = at.0;
        }
        self.text_range.1 = at.1;
        self.text.push(ch);
    }

    fn flush(&mut self) {
        if !self.text.is_empty() {
            self.pieces.push(Piece::Text {
                text: mem::take(&mut self.text),
                style: self.style.clone(),
                range: self.text_range,
            });
        }
    }

    fn close_gradients(&mut self, idx: usize) {
        while self.grads.last().is_some_and(|m| m.stack_idx >= idx) {
            let mark = self.grads.pop().expect("loop condition saw a last element");
            apply_gradient(&mut self.pieces, &mark);
        }
    }

    /// Closes the tag at `idx` and everything opened after it.
    fn close_at(&mut self, idx: usize) {
        let restored = self.stack[idx].1.clone();
        self.flush();
        self.close_gradients(idx);
        self.stack.truncate(idx);
        self.style = restored;
    }
}

impl Parser<'_> {
    fn parse(mut self) -> Result<Vec<Piece>, ParseError> {
        let mut ctx = Ctx::default();

        while self.pos < self.chars.len() {
            match self.chars[self.pos] {
                '\\' if matches!(self.peek(1), Some('<' | '\\')) => {
                    ctx.push_text(self.chars[self.pos + 1], self.span(2));
                    self.pos += 2;
                }
                '{' if self.peek(1) == Some('{') => {
                    ctx.push_text('{', self.span(2));
                    self.pos += 2;
                }
                '}' if self.peek(1) == Some('}') => {
                    ctx.push_text('}', self.span(2));
                    self.pos += 2;
                }
                '{' => {
                    ctx.flush();
                    let (arg, kind, spec, range) = self.read_hole()?;
                    ctx.pieces.push(Piece::Hole {
                        arg,
                        kind,
                        spec,
                        style: ctx.style.clone(),
                        range,
                    });
                }
                '<' => {
                    let tag = self.read_tag()?;
                    let depth = ctx.stack.len();
                    self.apply_tag(&tag.parts, tag.range, &mut ctx)?;
                    if tag.self_closing && ctx.stack.len() > depth {
                        ctx.close_at(depth);
                    }
                }
                c => {
                    ctx.push_text(c, self.span(1));
                    self.pos += 1;
                }
            }
        }
        ctx.flush();
        ctx.close_gradients(0);
        Ok(ctx.pieces)
    }

    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    const fn span(&self, len: usize) -> (usize, usize) {
        (self.offset + self.pos, self.offset + self.pos + len)
    }

    fn read_hole(&mut self) -> Result<ParsedHole, ParseError> {
        let open = self.offset + self.pos;
        self.pos += 1;
        let kind = if self.chars.get(self.pos) == Some(&'@') {
            self.pos += 1;
            HoleKind::Component
        } else {
            HoleKind::Text
        };
        let start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos] != '}' {
            self.pos += 1;
        }
        if self.pos >= self.chars.len() {
            return Err(
                ParseError::new((open, self.offset + self.chars.len()), "unclosed `{`")
                    .help("close the hole with `}`, or write `{{` for a literal brace"),
            );
        }
        let raw: String = self.chars[start..self.pos].iter().collect();
        self.pos += 1;
        let range = (open, self.offset + self.pos);
        if is_const_name(&raw) {
            let kind = if kind == HoleKind::Component {
                HoleKind::ConstComponent
            } else {
                HoleKind::ConstText
            };
            return self.const_hole(kind, raw["const".len()..].trim(), range);
        }
        let (name, spec) = self.split_spec(&raw, range)?;
        if kind == HoleKind::Component && spec.is_some() {
            return Err(
                ParseError::new(range, "component holes don't take a format spec").help(
                    "format specs apply to text holes — write `{name:…}` without the `@`, \
                     or drop the `:…`",
                ),
            );
        }
        let arg = self.hole_arg(name, range)?;
        Ok((arg, kind, spec, range))
    }

    fn const_hole(
        &mut self,
        kind: HoleKind,
        name: &str,
        range: (usize, usize),
    ) -> Result<ParsedHole, ParseError> {
        let component = kind == HoleKind::ConstComponent;
        if !self.mode.allow_const_holes() {
            return Err(if component {
                ParseError::new(range, "`{@const …}` only works in the text_nbt! macro").help(
                    "runtime templates cannot reference consts. Write {@name} and pass \
                     the component as a value",
                )
            } else {
                ParseError::new(
                    range,
                    "`{const …}` only works in the text! / text_nbt! macros",
                )
                .help("runtime templates cannot reference consts. Write {name} and pass the value")
            });
        }
        if name.is_empty() {
            return Err(if component {
                ParseError::new(range, "`{@const}` needs the name of a const")
                    .help("write {@const HEADER}, where HEADER is a `const EncodedComponent`")
            } else {
                ParseError::new(range, "`{const}` needs the name of a const")
                    .help("write {const GIT_HASH}, where GIT_HASH is a `const &str`")
            });
        }
        if name.contains(':') {
            return Err(
                ParseError::new(range, "const holes don't take a format spec")
                    .help("a const is spliced as it is written: drop the `:…`"),
            );
        }
        let arg = self.plain_hole_arg(name, range)?;
        Ok((arg, kind, None, range))
    }

    /// `{x:.2}` → `("x", Some(".2"))`.
    fn split_spec<'b>(
        &self,
        inner: &'b str,
        range: (usize, usize),
    ) -> Result<(&'b str, Option<String>), ParseError> {
        match inner.split_once(':') {
            None => Ok((inner, None)),
            Some((name, spec)) => {
                if !self.mode.allow_specs() {
                    return Err(ParseError::new(
                        range,
                        "format specs only work in the text! / text_nbt! macros",
                    )
                    .help(
                        "drop the `:…`; runtime templates fill holes with values that are \
                         already formatted",
                    ));
                }
                Ok((name, Some(spec.to_string())))
            }
        }
    }

    fn hole_arg(&mut self, name: &str, range: (usize, usize)) -> Result<HoleArg, ParseError> {
        if is_const_name(name) {
            return Err(ParseError::new(range, "`{const …}` can't fill this value")
                .help("const holes only work in text position"));
        }
        self.plain_hole_arg(name, range)
    }

    fn plain_hole_arg(&mut self, name: &str, range: (usize, usize)) -> Result<HoleArg, ParseError> {
        if name.is_empty() {
            if !self.mode.allow_positional() {
                return Err(
                    ParseError::new(range, "runtime templates need named holes").help(
                        "write {victim} instead of {} — positional holes only exist in the \
                     text! macro",
                    ),
                );
            }
            let n = *self.positional;
            *self.positional += 1;
            Ok(HoleArg::Positional(n))
        } else if is_ident(name) {
            Ok(HoleArg::Named {
                name: name.to_string(),
                range,
            })
        } else {
            Err(
                ParseError::new(range, format!("`{{{name}}}` is not a valid hole")).help(
                    "use `{}` for a positional argument or `{identifier}` to capture a variable",
                ),
            )
        }
    }

    /// `parts[0]` is the tag head; an argument is either fully unquoted or one `'…'` / `"…"` span.
    /// The flag reports a self-closing `<tag/>`.
    fn read_tag(&mut self) -> Result<ParsedTag, ParseError> {
        let open = self.offset + self.pos;
        self.pos += 1;
        let mut parts: Vec<TagArg> = Vec::new();
        let mut text = String::new();
        let mut arg_start = self.pos;
        let mut state = ArgState::Fresh;
        let mut self_closing = false;

        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if let ArgState::Quoted {
                quote,
                content_start,
                ..
            } = state
            {
                match c {
                    '\\' if matches!(self.peek(1), Some(next) if next == quote || next == '\\') => {
                        text.push(self.chars[self.pos + 1]);
                        self.pos += 2;
                    }
                    c if c == quote => {
                        state = ArgState::Closed {
                            quote,
                            range: (self.offset + content_start, self.offset + self.pos),
                        };
                        self.pos += 1;
                    }
                    c => {
                        text.push(c);
                        self.pos += 1;
                    }
                }
                continue;
            }
            match c {
                ':' | '>' => {
                    let range = match state {
                        ArgState::Closed { range, .. } => range,
                        _ => (self.offset + arg_start, self.offset + self.pos),
                    };
                    parts.push(TagArg {
                        text: mem::take(&mut text),
                        range,
                    });
                    self.pos += 1;
                    if c == '>' {
                        return Ok(ParsedTag {
                            parts,
                            range: (open, self.offset + self.pos),
                            self_closing,
                        });
                    }
                    arg_start = self.pos;
                    state = ArgState::Fresh;
                }
                '/' if self.peek(1) == Some('>') => {
                    self_closing = true;
                    self.pos += 1;
                }
                '\'' | '"' if matches!(state, ArgState::Fresh) => {
                    state = ArgState::Quoted {
                        quote: c,
                        open: self.pos,
                        content_start: self.pos + 1,
                    };
                    self.pos += 1;
                }
                _ => {
                    if let ArgState::Closed { quote, .. } = state {
                        return Err(self.stray_quote(quote));
                    }
                    if c == '\'' || c == '"' {
                        return Err(self.stray_quote(c));
                    }
                    state = ArgState::Unquoted;
                    text.push(c);
                    self.pos += 1;
                }
            }
        }

        if let ArgState::Quoted { quote, open, .. } = state {
            let at = self.offset + open;
            return Err(ParseError::new(
                (at, at + 1),
                format!("unclosed `{quote}` inside this tag"),
            )
            .help(format!(
                "close the quoted value, e.g. <hover:show_text:{quote}Hello{quote}>"
            )));
        }
        Err(
            ParseError::new((open, self.offset + self.chars.len()), "unclosed `<` tag")
                .help("close the tag with `>`, or escape a literal `<` as `\\<`"),
        )
    }

    fn stray_quote(&self, quote: char) -> ParseError {
        let at = self.offset + self.pos;
        ParseError::new((at, at + 1), "a quoted value must span the whole argument").help(format!(
            "a `{quote}` either opens the whole argument or is written `\\{quote}` for a \
             literal quote inside a quoted value"
        ))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match arm per tag/variant; splitting hides the shape"
    )]
    fn apply_tag(
        &mut self,
        parts: &[TagArg],
        range: (usize, usize),
        ctx: &mut Ctx,
    ) -> Result<(), ParseError> {
        let raw_head = parts[0].text.as_str();
        let lowered = raw_head.to_ascii_lowercase();
        let head = if raw_head.starts_with('{') {
            raw_head
        } else {
            lowered.as_str()
        };
        let args = &parts[1..];

        if let Some(name) = head.strip_prefix('/').filter(|_| args.is_empty()) {
            let canonical = canonical_tag(name);
            let Some(idx) = ctx.stack.iter().rposition(|(n, _)| n == &canonical) else {
                let help = if ctx.stack.is_empty() {
                    "no tags are open here — remove this, or add an opening tag before it"
                        .to_string()
                } else {
                    format!(
                        "currently open: {}",
                        ctx.stack
                            .iter()
                            .map(|(n, _)| format!("<{n}>"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                return Err(ParseError::new(
                    range,
                    format!("closing tag `</{name}>` doesn't match any open tag"),
                )
                .help(help));
            };
            ctx.close_at(idx);
            return Ok(());
        }

        if args.is_empty() {
            match head {
                "newline" | "br" => {
                    ctx.push_text('\n', range);
                    return Ok(());
                }
                "reset" => {
                    ctx.flush();
                    ctx.close_gradients(0);
                    ctx.stack.clear();
                    ctx.style = Style::default();
                    return Ok(());
                }
                _ => {}
            }

            if let Some(inner) = head.strip_prefix('{') {
                let Some(name) = inner.strip_suffix('}') else {
                    return Err(ParseError::new(range, "unclosed `{` inside this tag")
                        .help("dynamic colors are written <{}> or <{color_var}>"));
                };
                let arg = self.hole_arg(name, range)?;
                let mut new = ctx.style.clone();
                new.color = Some(ColorIr::Dyn(arg));
                ctx.flush();
                ctx.stack.push(("color".to_string(), ctx.style.clone()));
                ctx.style = new;
                return Ok(());
            }
        }

        let mut new = ctx.style.clone();
        let (name, negated) = match head.strip_prefix('!') {
            Some(rest) => (rest, true),
            None => (head, false),
        };
        let canonical = canonical_tag(name);

        let is_flag = matches!(
            canonical.as_str(),
            "bold" | "italic" | "underlined" | "strikethrough" | "obfuscated"
        );
        if negated && !is_flag && canonical != "shadow" {
            return Err(ParseError::new(
                range,
                format!("`<!{name}>` — only formatting flags and the shadow can be negated"),
            )
            .help("negation works on b/i/u/st/obf and shadow, e.g. <!bold>"));
        }
        if is_flag {
            let value = if negated {
                if !args.is_empty() {
                    return Err(ParseError::new(
                        range,
                        format!("`<!{name}:...>` can't take a value"),
                    )
                    .help(format!("write <!{name}> or <{name}:false>")));
                }
                BoolIr::Const(false)
            } else {
                self.flag_value(name, args, range)?
            };
            match canonical.as_str() {
                "bold" => new.bold = Some(value),
                "italic" => new.italic = Some(value),
                "underlined" => new.underlined = Some(value),
                "strikethrough" => new.strikethrough = Some(value),
                "obfuscated" => new.obfuscated = Some(value),
                _ => unreachable!(),
            }
            ctx.flush();
            ctx.stack.push((canonical, ctx.style.clone()));
            ctx.style = new;
            return Ok(());
        }

        // these produce a piece or mark a span rather than styling, so they return early
        match canonical.as_str() {
            "key" => {
                let [key] = args else {
                    return Err(ParseError::new(range, "`<key:…>` takes exactly one key")
                        .help("for example <key:key.jump>"));
                };
                if key.text.is_empty() {
                    return Err(ParseError::new(range, "`<key:>` needs a key")
                        .help("for example <key:key.inventory>"));
                }
                ctx.flush();
                ctx.pieces.push(Piece::Keybind {
                    key: key.text.clone(),
                    style: ctx.style.clone(),
                    range,
                });
                return Ok(());
            }
            "lang" | "lang_or" => {
                let [key, rest @ ..] = args else {
                    return Err(ParseError::new(range, "`<lang:…>` needs a translation key")
                        .help("for example <lang:multiplayer.player.left:'{@}'>"));
                };
                let key = if is_hole(&key.text) {
                    let inner = &key.text[1..key.text.len() - 1];
                    if inner.starts_with('@') {
                        return Err(
                            ParseError::new(key.range, "`{@…}` is a component hole").help(
                                "the key hole takes a Translation: <lang:{}:…> or <lang:{msg}:…>",
                            ),
                        );
                    }
                    LangKey::Dyn(self.hole_arg(inner, key.range)?)
                } else if key.text.is_empty() {
                    return Err(ParseError::new(range, "`<lang:>` needs a translation key")
                        .help("for example <lang:multiplayer.player.left>"));
                } else if key.text.starts_with('{') || key.text.ends_with('}') {
                    return Err(
                        ParseError::new(key.range, "malformed hole in the key position")
                            .help("a dynamic key is written <lang:{}:…> or <lang:{msg}:…>"),
                    );
                } else {
                    LangKey::Lit(key.text.clone())
                };
                let (fallback, lang_args) = if canonical == "lang_or" {
                    let [fallback, lang_args @ ..] = rest else {
                        return Err(ParseError::new(range, "`<lang_or:…>` needs a fallback")
                            .help("for example <lang_or:my.key:'Fallback text'>"));
                    };
                    (
                        Some(self.parse_str_segments(&fallback.text, fallback.range.0)?),
                        lang_args,
                    )
                } else {
                    (None, rest)
                };
                let mut parsed_args = Vec::with_capacity(lang_args.len());
                for arg in lang_args {
                    parsed_args.push(
                        Parser {
                            chars: arg.text.chars().collect(),
                            pos: 0,
                            mode: self.mode,
                            positional: &mut *self.positional,
                            offset: arg.range.0,
                        }
                        .parse()?,
                    );
                }
                ctx.flush();
                ctx.pieces.push(Piece::Lang {
                    key,
                    fallback,
                    args: parsed_args,
                    style: ctx.style.clone(),
                    range,
                });
                return Ok(());
            }
            "gradient" | "rainbow" => {
                let stops = if canonical == "rainbow" {
                    if !args.is_empty() {
                        return Err(ParseError::new(range, "`<rainbow>` takes no arguments")
                            .help("for explicit colors use <gradient:#5e4fa2:#f79459>"));
                    }
                    Vec::new()
                } else {
                    if args.len() < 2 {
                        return Err(ParseError::new(
                            range,
                            "`<gradient:…>` needs at least two colors",
                        )
                        .help("for example <gradient:#5e4fa2:#f79459> or <gradient:red:blue>"));
                    }
                    args.iter()
                        .map(|a| parse_stop(&a.text, a.range))
                        .collect::<Result<Vec<_>, _>>()?
                };
                ctx.flush();
                ctx.grads.push(GradMark {
                    stack_idx: ctx.stack.len(),
                    piece_start: ctx.pieces.len(),
                    stops,
                });
                ctx.stack.push((canonical, ctx.style.clone()));
                return Ok(());
            }
            "score" | "selector" | "nbt" | "sprite" | "head" => {
                let style = ctx.style.clone();
                let piece = match canonical.as_str() {
                    "score" => score_piece(args, range, style)?,
                    "selector" => self.selector_piece(args, range, style)?,
                    "nbt" => self.nbt_piece(args, range, style)?,
                    "sprite" => sprite_piece(args, range, style)?,
                    _ => head_piece(args, range, style)?,
                };
                ctx.flush();
                ctx.pieces.push(piece);
                return Ok(());
            }
            _ => {}
        }

        match canonical.as_str() {
            "color" if head.starts_with('#') => {
                if !args.is_empty() {
                    return Err(ParseError::new(range, "`<#RRGGBB>` takes no arguments")
                        .help("write the color on its own: <#ff8800>"));
                }
                new.color = Some(parse_hex(&head[1..], range)?);
            }
            "color" => {
                let [value] = args else {
                    return Err(
                        ParseError::new(range, "`<color:…>` takes exactly one color")
                            .help(COLOR_HELP),
                    );
                };
                new.color = Some(self.color_value(value)?);
            }
            "transition" => {
                new.color = Some(transition_color(args, range)?);
            }
            "font" => {
                // a resource location keeps its own colons: rejoin the split
                let font = args
                    .iter()
                    .map(|a| a.text.as_str())
                    .collect::<Vec<_>>()
                    .join(":");
                if font.is_empty() {
                    return Err(ParseError::new(range, "`<font:>` needs a font")
                        .help("for example <font:minecraft:uniform>"));
                }
                new.font = Some(font);
            }
            "shadow" if negated => {
                if !args.is_empty() {
                    return Err(ParseError::new(range, "`<!shadow:...>` can't take a value")
                        .help("write <!shadow> to turn the shadow off"));
                }
                new.shadow_color = Some(0);
            }
            "shadow" => {
                let (value, alpha) = match args {
                    [value] => (value, None),
                    [value, alpha] => (value, Some(alpha)),
                    _ => {
                        return Err(ParseError::new(
                            range,
                            "`<shadow:…>` takes a color and an optional alpha",
                        )
                        .help(
                            "for example <shadow:#80000000> (ARGB), <shadow:red> or \
                             <shadow:red:0.5>",
                        ));
                    }
                };
                new.shadow_color = Some(parse_shadow(value, alpha)?);
            }
            "insert" => {
                let [value] = args else {
                    return Err(ParseError::new(
                        range,
                        "`<insert:…>` takes exactly one quoted value",
                    )
                    .help("for example <insert:'/msg {name} '>"));
                };
                new.insertion = Some(self.parse_str_segments(&value.text, value.range.0)?);
            }
            "hover" if is_event_hole(args) => {
                new.hover = Some(HoverIr::Dyn(self.event_hole_arg(&args[0])?));
            }
            "hover" => {
                new.hover = Some(self.hover_value(args, range)?);
            }
            "click" if is_event_hole(args) => {
                new.click = Some(ClickIr::Dyn(self.event_hole_arg(&args[0])?));
            }
            "click" => {
                const ACTIONS: [&str; 6] = [
                    "open_url",
                    "run_command",
                    "suggest_command",
                    "copy_to_clipboard",
                    "change_page",
                    "show_dialog",
                ];
                let [action, value] = expect_args(args, range)?;
                let segs = self.parse_str_segments(&value.text, value.range.0)?;
                let all_lit = StrSeg::join_lits(&segs);
                let kind = match action.text.as_str() {
                    "open_url" => {
                        // holes make the url runtime data — only validate literals
                        if let Some(url) = &all_lit
                            && !url.starts_with("http://")
                            && !url.starts_with("https://")
                        {
                            return Err(ParseError::new(
                                range,
                                format!("`{url}` is not a url the client will open"),
                            )
                            .help("vanilla clients only open http:// and https:// urls"));
                        }
                        ClickKind::OpenUrl
                    }
                    "run_command" => ClickKind::RunCommand,
                    "suggest_command" => ClickKind::SuggestCommand,
                    "copy_to_clipboard" => ClickKind::CopyToClipboard,
                    "show_dialog" => ClickKind::ShowDialog,
                    "change_page" => {
                        let Some(page) = &all_lit else {
                            return Err(ParseError::new(range, "change_page can't take holes")
                                .help("change_page takes a literal page number"));
                        };
                        ClickKind::ChangePage(page.parse().map_err(|_| {
                            ParseError::new(range, format!("`{page}` is not a valid page number"))
                                .help("change_page takes an integer: <click:change_page:'3'>")
                        })?)
                    }
                    other => {
                        let err = ParseError::new(range, format!("unknown click action `{other}`"));
                        return Err(match suggest(other, ACTIONS) {
                            Some(s) => err.help(format!("did you mean `{s}`?")),
                            None => err.help(format!("valid actions: {}", ACTIONS.join(", "))),
                        });
                    }
                };
                new.click = Some(ClickIr::Action(kind, segs));
            }
            named if NAMED_COLORS.iter().any(|(_, v, _)| *v == named) => {
                if !args.is_empty() {
                    return Err(
                        ParseError::new(range, format!("`<{name}>` takes no arguments"))
                            .help(format!("write the color on its own: <{name}>")),
                    );
                }
                new.color = Some(ColorIr::Named(
                    NAMED_COLORS
                        .iter()
                        .find(|(_, v, _)| *v == named)
                        .expect("the match guard just found this name")
                        .1,
                ));
            }
            other => {
                let err = ParseError::new(range, format!("unknown tag `<{other}>`"));
                return Err(match suggest(other, all_tag_names()) {
                    Some(s) => err.help(format!("did you mean `<{s}>`?")),
                    None => err.help(SUPPORTED_SUMMARY),
                });
            }
        }
        ctx.flush();
        ctx.stack.push((canonical, ctx.style.clone()));
        ctx.style = new;
        Ok(())
    }

    fn event_hole_arg(&mut self, arg: &TagArg) -> Result<HoleArg, ParseError> {
        let inner = &arg.text[1..arg.text.len() - 1];
        if inner.starts_with('@') {
            return Err(ParseError::new(arg.range, "`{@…}` is a component hole")
                .help("an event hole takes the event itself: <hover:{item}>"));
        }
        self.hole_arg(inner, arg.range)
    }

    fn color_value(&mut self, arg: &TagArg) -> Result<ColorIr, ParseError> {
        let value = arg.text.as_str();
        if is_hole(value) {
            return Ok(ColorIr::Dyn(
                self.hole_arg(&value[1..value.len() - 1], arg.range)?,
            ));
        }
        if let Some(hex) = value.strip_prefix('#') {
            return parse_hex(hex, arg.range);
        }
        if let Some((_, canonical, _)) = NAMED_COLORS.iter().find(|(alias, ..)| *alias == value) {
            Ok(ColorIr::Named(canonical))
        } else {
            let err = ParseError::new(arg.range, format!("`{value}` is not a color"));
            let aliases = NAMED_COLORS.iter().map(|(alias, ..)| *alias);
            Err(match suggest(value, aliases) {
                Some(s) => err.help(format!("did you mean `{s}`? {COLOR_HELP}")),
                None => err.help(COLOR_HELP),
            })
        }
    }

    /// Parses a tag argument as a template of its own.
    fn nested(&mut self, arg: &TagArg) -> Result<Vec<Piece>, ParseError> {
        Parser {
            chars: arg.text.chars().collect(),
            pos: 0,
            mode: self.mode,
            positional: &mut *self.positional,
            offset: arg.range.0,
        }
        .parse()
    }

    fn hover_value(
        &mut self,
        args: &[TagArg],
        range: (usize, usize),
    ) -> Result<HoverIr, ParseError> {
        const ACTIONS: [&str; 3] = ["show_text", "show_item", "show_entity"];
        let [action, values @ ..] = args else {
            return Err(
                ParseError::new(range, "`<hover:…>` needs an action and a value")
                    .help("for example <hover:show_text:'Hi'>"),
            );
        };
        match action.text.as_str() {
            "show_text" => {
                let [value] = values else {
                    return Err(ParseError::new(
                        range,
                        "`<hover:show_text:…>` takes exactly one value",
                    )
                    .help("for example <hover:show_text:'Hi'>"));
                };
                let inner = self.nested(value)?;
                if inner.is_empty() {
                    return Err(ParseError::new(range, "empty hover text")
                        .help("give the hover something to show: <hover:show_text:'Hi'>"));
                }
                Ok(HoverIr::Text(inner))
            }
            "show_item" => show_item(values, range),
            "show_entity" => self.show_entity(values, range),
            other => {
                let err = ParseError::new(range, format!("unknown hover action `{other}`"));
                Err(match suggest(other, ACTIONS) {
                    Some(s) => err.help(format!("did you mean `{s}`?")),
                    None => err.help(format!("valid actions: {}", ACTIONS.join(", "))),
                })
            }
        }
    }

    fn show_entity(
        &mut self,
        values: &[TagArg],
        range: (usize, usize),
    ) -> Result<HoverIr, ParseError> {
        let (id, uuid, name) = match values {
            [id, uuid] => (id, uuid, None),
            [id, uuid, name] => (id, uuid, Some(name)),
            _ => {
                return Err(ParseError::new(
                    range,
                    "`<hover:show_entity:…>` needs an entity type and a UUID",
                )
                .help(UUID_HELP));
            }
        };
        let id = literal_arg("hover", id)?;
        if id.is_empty() {
            return Err(
                ParseError::new(range, "`<hover:show_entity:>` needs an entity type")
                    .help(UUID_HELP),
            );
        }
        let Some(uuid) = parse_uuid(literal_arg("hover", uuid)?) else {
            return Err(
                ParseError::new(uuid.range, format!("`{}` is not a UUID", uuid.text))
                    .help(UUID_HELP),
            );
        };
        let name = match name {
            None => None,
            Some(name) => {
                let inner = self.nested(name)?;
                if inner.is_empty() {
                    return Err(ParseError::new(name.range, "empty entity name")
                        .help("drop the argument to show no name"));
                }
                Some(inner)
            }
        };
        Ok(HoverIr::Entity {
            id: id.to_string(),
            uuid,
            name,
        })
    }

    fn selector_piece(
        &mut self,
        args: &[TagArg],
        range: (usize, usize),
        style: Style,
    ) -> Result<Piece, ParseError> {
        let (selector, separator) = match args {
            [selector] => (selector, None),
            [selector, separator] => (selector, Some(separator)),
            _ => {
                return Err(ParseError::new(
                    range,
                    "`<selector:…>` takes a selector and an optional separator",
                )
                .help("for example <selector:@a> or <selector:@a:', '>"));
            }
        };
        let selector = literal_arg("selector", selector)?;
        if selector.is_empty() {
            return Err(ParseError::new(range, "`<selector:>` needs a selector")
                .help("for example <selector:@a>"));
        }
        let separator = match separator {
            None => None,
            Some(separator) => Some(self.nested(separator)?),
        };
        Ok(Piece::Selector {
            selector: selector.to_string(),
            separator,
            style,
            range,
        })
    }

    fn nbt_piece(
        &mut self,
        args: &[TagArg],
        range: (usize, usize),
        style: Style,
    ) -> Result<Piece, ParseError> {
        const SOURCES: [&str; 3] = ["block", "entity", "storage"];
        let [source, id, path, rest @ ..] = args else {
            return Err(
                ParseError::new(range, "`<nbt:…>` needs a source, an id and a path")
                    .help("for example <nbt:entity:'@s':Health>"),
            );
        };
        let id = literal_arg("nbt", id)?.to_string();
        let path = literal_arg("nbt", path)?;
        if id.is_empty() || path.is_empty() {
            return Err(ParseError::new(range, "`<nbt:…>` needs an id and a path")
                .help("for example <nbt:storage:'my:key':path>"));
        }
        let source = match literal_arg("nbt", source)?.to_ascii_lowercase().as_str() {
            "block" => NbtSourceIr::Block(id),
            "entity" => NbtSourceIr::Entity(id),
            "storage" => NbtSourceIr::Storage(id),
            other => {
                let err = ParseError::new(source.range, format!("`{other}` is not an NBT source"));
                return Err(match suggest(other, SOURCES) {
                    Some(s) => err.help(format!("did you mean `{s}`?")),
                    None => err.help(format!("valid sources: {}", SOURCES.join(", "))),
                });
            }
        };
        let (separator, interpret) = match rest {
            [] => (None, false),
            [flag] if flag.text.eq_ignore_ascii_case("interpret") => (None, true),
            [separator] => (Some(self.nested(separator)?), false),
            [separator, flag] if flag.text.eq_ignore_ascii_case("interpret") => {
                (Some(self.nested(separator)?), true)
            }
            _ => {
                return Err(ParseError::new(
                    range,
                    "`<nbt:…>` takes a separator and `interpret` after the path",
                )
                .help("for example <nbt:entity:'@s':Health:', ':interpret>"));
            }
        };
        Ok(Piece::Nbt {
            source,
            path: path.to_string(),
            interpret,
            separator,
            style,
            range,
        })
    }

    fn parse_str_segments(
        &mut self,
        value: &str,
        offset: usize,
    ) -> Result<Vec<StrSeg>, ParseError> {
        let chars: Vec<char> = value.chars().collect();
        let mut segs: Vec<StrSeg> = Vec::new();
        let mut lit = String::new();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '{' if chars.get(i + 1) == Some(&'{') => {
                    lit.push('{');
                    i += 2;
                }
                '}' if chars.get(i + 1) == Some(&'}') => {
                    lit.push('}');
                    i += 2;
                }
                '{' => {
                    let open = offset + i;
                    let Some(close) = (i + 1..chars.len()).find(|&j| chars[j] == '}') else {
                        return Err(ParseError::new(
                            (open, offset + chars.len()),
                            "unclosed `{` in this value",
                        )
                        .help("close the hole with `}`, or write `{{` for a literal brace"));
                    };
                    let raw: String = chars[i + 1..close].iter().collect();
                    let range = (open, offset + close + 1);
                    let (name, spec) = self.split_spec(&raw, range)?;
                    let arg = self.hole_arg(name, range)?;
                    if !lit.is_empty() {
                        segs.push(StrSeg::Lit(mem::take(&mut lit)));
                    }
                    segs.push(StrSeg::Hole(arg, spec));
                    i = close + 1;
                }
                c => {
                    lit.push(c);
                    i += 1;
                }
            }
        }
        if !lit.is_empty() {
            segs.push(StrSeg::Lit(lit));
        }
        Ok(segs)
    }

    fn flag_value(
        &mut self,
        name: &str,
        args: &[TagArg],
        range: (usize, usize),
    ) -> Result<BoolIr, ParseError> {
        match args {
            [] => Ok(BoolIr::Const(true)),
            [v] if v.text == "true" => Ok(BoolIr::Const(true)),
            [v] if v.text == "false" => Ok(BoolIr::Const(false)),
            [v] if is_hole(&v.text) => {
                let inner = &v.text[1..v.text.len() - 1];
                Ok(BoolIr::Dyn(self.hole_arg(inner, v.range)?))
            }
            [v] => Err(ParseError::new(
                v.range,
                format!("`{}` is not a valid value for <{name}:...>", v.text),
            )
            .help(format!(
                "use <{name}>, <{name}:false>, or <{name}:{{}}> for a runtime bool"
            ))),
            _ => Err(
                ParseError::new(range, format!("<{name}:...> takes at most one value"))
                    .help(format!("for example <{name}:{{is_op}}>")),
            ),
        }
    }
}

fn expect_args(args: &[TagArg], range: (usize, usize)) -> Result<&[TagArg; 2], ParseError> {
    args.try_into().map_err(|_| {
        ParseError::new(
            range,
            "`<click:...>` needs exactly an action and a quoted value",
        )
        .help("for example: <click:run_command:'…'>")
    })
}

const UUID_HELP: &str = "for example <hover:show_entity:pig:1f085b2d-9548-4159-a8c7-f3ccdf0c2054>";

/// Rejects a hole in an argument the tag reads literally.
fn literal_arg<'b>(tag: &str, arg: &'b TagArg) -> Result<&'b str, ParseError> {
    if is_hole(&arg.text) {
        return Err(
            ParseError::new(arg.range, format!("`<{tag}:…>` doesn't take holes"))
                .help("build the component and pass it through a {@} hole"),
        );
    }
    Ok(arg.text.as_str())
}

/// `8-4-4-4-12` or 32 plain hex digits, as big-endian bytes.
fn parse_uuid(value: &str) -> Option<[u8; 16]> {
    let digits: Vec<u8> = if value.len() == 36 {
        let bytes = value.as_bytes();
        if [8, 13, 18, 23].iter().any(|&i| bytes[i] != b'-') {
            return None;
        }
        bytes.iter().copied().filter(|b| *b != b'-').collect()
    } else {
        value.as_bytes().to_vec()
    };
    if digits.len() != 32 || !digits.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        let pair = str::from_utf8(&digits[i * 2..i * 2 + 2]).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

fn show_item(values: &[TagArg], range: (usize, usize)) -> Result<HoverIr, ParseError> {
    let (id, count) = match values {
        [id] => (id, None),
        [id, count] => (id, Some(count)),
        _ => {
            return Err(ParseError::new(range, "item components need runtime data")
                .help("build the HoverEvent and pass it through a hole: <hover:{item}>"));
        }
    };
    let id = literal_arg("hover", id)?;
    if id.is_empty() {
        return Err(
            ParseError::new(range, "`<hover:show_item:>` needs an item id")
                .help("for example <hover:show_item:'minecraft:diamond_sword':3>"),
        );
    }
    let count = match count {
        None => 1,
        Some(count) => literal_arg("hover", count)?.parse().map_err(|_| {
            ParseError::new(
                count.range,
                format!("`{}` is not an item count", count.text),
            )
            .help("the count is an integer: <hover:show_item:'minecraft:stone':3>")
        })?,
    };
    Ok(HoverIr::Item {
        id: id.to_string(),
        count,
    })
}

fn score_piece(args: &[TagArg], range: (usize, usize), style: Style) -> Result<Piece, ParseError> {
    let [name, objective] = args else {
        return Err(
            ParseError::new(range, "`<score:…>` takes a holder and an objective")
                .help("for example <score:@s:kills>"),
        );
    };
    let name = literal_arg("score", name)?;
    let objective = literal_arg("score", objective)?;
    if name.is_empty() || objective.is_empty() {
        return Err(
            ParseError::new(range, "`<score:…>` needs a holder and an objective")
                .help("for example <score:@s:kills>"),
        );
    }
    Ok(Piece::Score {
        name: name.to_string(),
        objective: objective.to_string(),
        style,
        range,
    })
}

fn sprite_piece(args: &[TagArg], range: (usize, usize), style: Style) -> Result<Piece, ParseError> {
    let (atlas, sprite) = match args {
        [sprite] => ("minecraft:blocks", literal_arg("sprite", sprite)?),
        [atlas, sprite] => (
            literal_arg("sprite", atlas)?,
            literal_arg("sprite", sprite)?,
        ),
        _ => {
            return Err(ParseError::new(
                range,
                "`<sprite:…>` takes an optional atlas and a sprite",
            )
            .help("quote a namespaced id: <sprite:'minecraft:items':item/emerald>"));
        }
    };
    if atlas.is_empty() || sprite.is_empty() {
        return Err(ParseError::new(range, "`<sprite:>` needs a sprite")
            .help("for example <sprite:item/emerald>"));
    }
    Ok(Piece::Sprite {
        atlas: atlas.to_string(),
        sprite: sprite.to_string(),
        style,
        range,
    })
}

fn head_piece(args: &[TagArg], range: (usize, usize), style: Style) -> Result<Piece, ParseError> {
    let (who, outer_layer) = match args {
        [who] => (who, None),
        [who, outer_layer] => (who, Some(outer_layer)),
        _ => {
            return Err(ParseError::new(
                range,
                "`<head:…>` takes a player and an optional outer layer",
            )
            .help("for example <head:Notch> or <head:Notch:false>"));
        }
    };
    let text = literal_arg("head", who)?;
    let player = if let Some(uuid) = parse_uuid(text) {
        HeadIr::Uuid(uuid)
    } else if text.contains(':') || text.contains('/') {
        HeadIr::Texture(text.to_string())
    } else {
        if text.is_empty()
            || text.encode_utf16().count() > 16
            || text.chars().any(|c| c <= ' ' || c >= '\u{7f}')
        {
            return Err(
                ParseError::new(who.range, format!("`{text}` is not a player")).help(
                    "a head takes a player name, a UUID, or a texture: <head:Notch>, \
                     <head:1f085b2d-9548-4159-a8c7-f3ccdf0c2054>, \
                     <head:'minecraft:textures/…'>",
                ),
            );
        }
        HeadIr::Name(text.to_string())
    };
    let hat = match outer_layer {
        None => true,
        Some(value) if value.text == "true" => true,
        Some(value) if value.text == "false" => false,
        Some(value) => {
            return Err(ParseError::new(
                value.range,
                format!("`{}` is not an outer layer flag", value.text),
            )
            .help("the outer layer is true or false: <head:Notch:false>"));
        }
    };
    Ok(Piece::Head {
        player,
        hat,
        style,
        range,
    })
}

/// `<transition:color…:phase>`: the stops mixed at `phase`, wrapping a negative one.
fn transition_color(args: &[TagArg], range: (usize, usize)) -> Result<ColorIr, ParseError> {
    let missing = || {
        ParseError::new(range, "`<transition:…>` needs at least one color")
            .help("for example <transition:red:blue:0.5>")
    };
    let [.., last] = args else {
        return Err(missing());
    };
    let (stop_args, phase) = match last.text.parse::<f32>() {
        Ok(phase) => {
            if !(-1.0..=1.0).contains(&phase) {
                return Err(phase_error(last));
            }
            (
                &args[..args.len() - 1],
                if phase < 0.0 { 1.0 + phase } else { phase },
            )
        }
        Err(_) => (args, 0.0),
    };
    if stop_args.is_empty() {
        return Err(missing());
    }
    let stops = stop_args
        .iter()
        .map(|a| parse_stop(&a.text, a.range))
        .collect::<Result<Vec<_>, _>>()?;
    let (r, g, b) = mix_stops(&stops, phase);
    Ok(ColorIr::Rgb(r, g, b))
}

fn phase_error(phase: &TagArg) -> ParseError {
    ParseError::new(
        phase.range,
        format!("`{}` is not a phase between -1 and 1", phase.text),
    )
    .help("the transition phase is a fraction: <transition:red:blue:0.5>")
}

fn is_event_hole(args: &[TagArg]) -> bool {
    matches!(args, [one] if is_hole(&one.text))
}

fn is_hole(text: &str) -> bool {
    text.starts_with('{') && text.ends_with('}') && text.len() >= 2
}

/// Whether a hole body is the `const` marker rather than a name.
fn is_const_name(name: &str) -> bool {
    name.strip_prefix("const")
        .is_some_and(|rest| rest.chars().next().is_none_or(char::is_whitespace))
}

/// Rust's own identifier rule: hole names become variables in the macros.
fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some('_') => !chars.as_str().is_empty() && chars.all(unicode_ident::is_xid_continue),
        Some(first) => {
            unicode_ident::is_xid_start(first) && chars.all(unicode_ident::is_xid_continue)
        }
        None => false,
    }
}

fn parse_stop(value: &str, range: (usize, usize)) -> Result<(u8, u8, u8), ParseError> {
    if let Some(hex) = value.strip_prefix('#') {
        return match parse_hex(hex, range)? {
            ColorIr::Rgb(r, g, b) => Ok((r, g, b)),
            _ => unreachable!(),
        };
    }
    NAMED_COLORS
        .iter()
        .find(|(alias, ..)| *alias == value)
        .map(|(.., rgb)| *rgb)
        .ok_or_else(|| {
            ParseError::new(range, format!("`{value}` is not a color"))
                .help("gradient stops are named colors or hex, e.g. <gradient:red:#f79459>")
        })
}

/// `#AARRGGBB`, `#RRGGBB` or a named color with an optional `0..1` alpha. `#RRGGBB` and named colors take `0.25` by default.
fn parse_shadow(value: &TagArg, alpha: Option<&TagArg>) -> Result<i32, ParseError> {
    let mut argb = if let Some(hex) = value.text.strip_prefix('#')
        && hex.len() == 8
    {
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ParseError::new(
                value.range,
                format!("`#{hex}` is not a valid shadow color"),
            )
            .help("shadow colors are <shadow:#AARRGGBB> or <shadow:#RRGGBB>"));
        }
        u32::from_str_radix(hex, 16).expect("eight ascii hex digits checked above")
    } else {
        let (r, g, b) = parse_stop(&value.text, value.range)?;
        0x4000_0000 | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
    };
    if let Some(alpha) = alpha {
        let fraction: f32 = alpha.text.parse().map_err(|_| alpha_error(alpha))?;
        if !(0.0..=1.0).contains(&fraction) {
            return Err(alpha_error(alpha));
        }
        argb = (argb & 0x00FF_FFFF) | ((fraction * 255.0).round() as u32) << 24;
    }
    Ok(argb as i32)
}

fn alpha_error(alpha: &TagArg) -> ParseError {
    ParseError::new(
        alpha.range,
        format!("`{}` is not an alpha between 0 and 1", alpha.text),
    )
    .help("the shadow alpha is a fraction: <shadow:red:0.5>")
}

impl GradMark {
    fn color_at(&self, i: usize, total: usize) -> ColorIr {
        if self.stops.is_empty() {
            // the hue sweeps the whole circle, so the last character must not repeat the first
            let (r, g, b) = hue_to_rgb(i as f32 / total as f32);
            return ColorIr::Rgb(r, g, b);
        }
        let fraction = if total == 1 {
            0.0
        } else {
            i as f32 / (total - 1) as f32
        };
        let (r, g, b) = mix_stops(&self.stops, fraction);
        ColorIr::Rgb(r, g, b)
    }
}

/// The color `fraction` of the way along a stop list.
fn mix_stops(stops: &[(u8, u8, u8)], fraction: f32) -> (u8, u8, u8) {
    if stops.len() == 1 {
        return stops[0];
    }
    let span = fraction * (stops.len() - 1) as f32;
    let lo = (span.floor() as usize).min(stops.len() - 2);
    let f = span - lo as f32;
    let (r0, g0, b0) = stops[lo];
    let (r1, g1, b1) = stops[lo + 1];
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * f).round() as u8;
    (mix(r0, r1), mix(g0, g1), mix(b0, b1))
}

fn hue_to_rgb(h: f32) -> (u8, u8, u8) {
    let h6 = (h.fract() * 6.0).rem_euclid(6.0);
    let sector = h6.floor() as u32;
    let f = h6 - sector as f32;
    let rise = (f * 255.0).round() as u8;
    let fall = ((1.0 - f) * 255.0).round() as u8;
    match sector {
        0 => (255, rise, 0),
        1 => (fall, 255, 0),
        2 => (0, 255, rise),
        3 => (0, fall, 255),
        4 => (rise, 0, 255),
        _ => (255, 0, fall),
    }
}

/// Colors a closed gradient span: text splits per character, other pieces take one position.
fn apply_gradient(pieces: &mut Vec<Piece>, mark: &GradMark) {
    let total: usize = pieces[mark.piece_start..].iter().map(Piece::width).sum();
    if total == 0 {
        return;
    }
    let span: Vec<Piece> = pieces.drain(mark.piece_start..).collect();
    let mut pos = 0usize;
    for piece in span {
        match piece {
            // escapes make the char index in `text` no guide to the source, so reuse its range
            Piece::Text { text, style, range } if style.color.is_none() => {
                for ch in text.chars() {
                    let mut style = style.clone();
                    style.color = Some(mark.color_at(pos, total));
                    pieces.push(Piece::Text {
                        text: ch.to_string(),
                        style,
                        range,
                    });
                    pos += 1;
                }
            }
            mut other => {
                if other.style().color.is_none() {
                    let color = mark.color_at(pos, total);
                    other.style_mut().color = Some(color);
                }
                pos += other.width();
                pieces.push(other);
            }
        }
    }
}

fn parse_hex(hex: &str, range: (usize, usize)) -> Result<ColorIr, ParseError> {
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(
            ParseError::new(range, format!("`#{hex}` is not a valid hex color"))
                .help("hex colors are six digits: <#RRGGBB>, e.g. <#ff8800>"),
        );
    }
    let v = u32::from_str_radix(hex, 16).expect("six ascii hex digits checked above");
    Ok(ColorIr::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}
