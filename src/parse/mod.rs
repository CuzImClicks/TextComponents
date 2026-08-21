use crate::{
    Modifier, TextComponent,
    content::{Content, NbtSource, Object, ObjectPlayer, Resolvable},
    format::{Color, Format},
    interactivity::{Interactivity, MaybeStatic},
    translation::{Args, TranslatedMessage},
};
use std::{borrow::Cow, iter::Peekable, ops::AddAssign, str::Chars};

mod error;
mod events;
#[cfg(feature = "nbt")]
pub(crate) mod nbt;
mod object;
mod scalar;

pub use error::{SnbtError, SnbtResult};

use events::{parse_click, parse_hover};
#[cfg(feature = "custom")]
use object::parse_custom;
use object::{parse_player, parse_scoreboard};
use scalar::{parse_bool, parse_num, parse_string};

impl TextComponent {
    /// Parses a component from SNBT.
    pub fn from_snbt(string: &str) -> SnbtResult<TextComponent> {
        parse_body(None, &mut string.chars().peekable())
    }
}

fn parse_body(first: Option<char>, chars: &mut Peekable<Chars>) -> SnbtResult<TextComponent> {
    let char = if let Some(first) = first {
        first
    } else {
        let mut first = ' ';
        for char in chars.by_ref() {
            if char.is_whitespace() {
                continue;
            }
            first = char;
            break;
        }
        if first == ' ' {
            return Err(SnbtError::EndedAbruptely(line!()));
        }
        first
    };

    match char {
        '"' => return parse_string('"', chars).map(TextComponent::plain),
        '\'' => return parse_string('\'', chars).map(TextComponent::plain),
        '[' => {
            let mut components = parse_vec(chars)?.into_iter();
            let first = components.next().ok_or(SnbtError::Required(
                "Lists".to_string(),
                "at least one component".to_string(),
            ))?;
            let children = components.collect();
            return Ok(first.add_children(children));
        }
        '{' => return parse_compound(chars),
        _ => (),
    }
    Err(SnbtError::EndedAbruptely(line!()))
}

fn parse_vec(chars: &mut Peekable<Chars>) -> SnbtResult<Vec<TextComponent>> {
    let mut component = vec![];
    let Ok(child) = parse_body(None, chars) else {
        return Err(SnbtError::UnfinishedComponent(line!()));
    };
    component.push(child);
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            ']' => return Ok(component),
            ',' => {
                let child = parse_body(None, chars)?;
                component.push(child);
            }
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}

struct CompoundParts {
    pub content: String,
    pub object: String,
    pub contents: [Option<Content>; 9],
    pub nbt: String,
    pub nbt_sources: [Option<NbtSource>; 3],
}
impl CompoundParts {
    pub const fn new() -> Self {
        CompoundParts {
            content: String::new(),
            object: String::new(),
            contents: [None, None, None, None, None, None, None, None, None],
            nbt: String::new(),
            nbt_sources: [None, None, None],
        }
    }
}

fn parse_compound(chars: &mut Peekable<Chars>) -> SnbtResult<TextComponent> {
    let mut compound = CompoundParts::new();
    let mut format = Format::new();
    let mut interactions = Interactivity::new();
    let mut children = vec![];
    let mut name = String::new();
    let mut in_name = true;
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            '}' => {
                return Ok(TextComponent {
                    content: retrieve_content(compound)?,
                    children: children.into(),
                    format,
                    interactions,
                });
            }
            ',' => in_name = true,
            '"' => {
                in_name = false;
                name = parse_string('"', chars)?;
            }
            '\'' => {
                in_name = false;
                name = parse_string('\'', chars)?;
            }
            ':' => {
                in_name = false;
                let mut unknown = 0u8;
                let mut first = ' ';
                for char in chars.by_ref() {
                    if char.is_whitespace() {
                        continue;
                    }
                    first = char;
                    break;
                }
                if first == ' ' {
                    return Err(SnbtError::EndedAbruptely(line!()));
                }
                if name == "extra" {
                    children = parse_vec(chars)?;
                    name = String::new();
                    continue;
                }
                match_content(&name, &mut compound, first, chars, &mut unknown)?;
                match_format(&name, &mut format, first, chars, &mut unknown)?;
                match_interactions(&name, &mut interactions, first, chars, &mut unknown)?;
                if unknown == 3 {
                    return Err(SnbtError::UnknownKey(name));
                }
                name = String::new();
            }
            ch if in_name => name.push(ch),
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}

#[expect(
    clippy::too_many_lines,
    reason = "one match arm per tag/variant; splitting hides the shape"
)]
fn match_content(
    name: &str,
    compound: &mut CompoundParts,
    first: char,
    chars: &mut Peekable<Chars>,
    unknown: &mut u8,
) -> SnbtResult<()> {
    match name {
        "type" => {
            if first == '\'' || first == '"' {
                compound.content = parse_string(first, chars)?;
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "text" => {
            if first == '\'' || first == '"' {
                compound.contents[0] = Some(Content::Text {
                    text: Cow::Owned(parse_string(first, chars)?),
                });
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "translate" => {
            if first == '\'' || first == '"' {
                if let Some(Content::Translate(msg)) = &mut compound.contents[1] {
                    msg.key = Cow::Owned(parse_string(first, chars)?);
                } else {
                    compound.contents[1] = Some(Content::Translate(TranslatedMessage {
                        key: Cow::Owned(parse_string(first, chars)?),
                        fallback: None,
                        args: Args::None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "fallback" => {
            if first == '\'' || first == '"' {
                if let Some(Content::Translate(msg)) = &mut compound.contents[1] {
                    msg.fallback = Some(parse_string(first, chars)?.into_boxed_str());
                } else {
                    compound.contents[1] = Some(Content::Translate(TranslatedMessage {
                        key: Cow::Borrowed(""),
                        fallback: Some(parse_string(first, chars)?.into_boxed_str()),
                        args: Args::None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "with" => {
            if first == '[' {
                if let Some(Content::Translate(msg)) = &mut compound.contents[1] {
                    msg.args = Args::Owned(parse_vec(chars)?.into_boxed_slice());
                } else {
                    compound.contents[1] = Some(Content::Translate(TranslatedMessage {
                        key: Cow::Borrowed(""),
                        fallback: None,
                        args: Args::Owned(parse_vec(chars)?.into_boxed_slice()),
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "score" => {
            if first == '{' {
                compound.contents[2] = Some(parse_scoreboard(chars)?);
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "selector" => {
            if first == '\'' || first == '"' {
                if let Some(Content::Resolvable(Resolvable::Entity { selector, .. })) =
                    &mut compound.contents[3]
                {
                    *selector = Cow::Owned(parse_string(first, chars)?);
                } else {
                    compound.contents[3] = Some(Content::Resolvable(Resolvable::Entity {
                        selector: Cow::Owned(parse_string(first, chars)?),
                        separator: None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "separator" => {
            if first == '\'' || first == '"' {
                if let Some(Content::Resolvable(Resolvable::Entity { separator, .. })) =
                    &mut compound.contents[3]
                {
                    *separator = Some(Box::new(parse_body(Some(first), chars)?));
                } else {
                    compound.contents[3] = Some(Content::Resolvable(Resolvable::Entity {
                        selector: Cow::Borrowed("-None-"),
                        separator: Some(Box::new(parse_body(Some(first), chars)?)),
                    }));
                }
                if let Some(Content::Resolvable(Resolvable::NBT { separator, .. })) =
                    &mut compound.contents[5]
                {
                    *separator = Some(Box::new(parse_body(Some(first), chars)?));
                } else {
                    compound.contents[5] = Some(Content::Resolvable(Resolvable::NBT {
                        path: Cow::Borrowed("-None-"),
                        interpret: false,
                        plain: false,
                        separator: Some(Box::new(parse_body(Some(first), chars)?)),
                        source: NbtSource::Block(Cow::Borrowed("")),
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "keybind" => {
            if first == '\'' || first == '"' {
                compound.contents[4] = Some(Content::Keybind {
                    keybind: Cow::Owned(parse_string(first, chars)?),
                });
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "source" => {
            if first == '\'' || first == '"' {
                compound.nbt = parse_string(first, chars)?;
                return match compound.object.as_str() {
                    "block" | "entity" | "storage" => Ok(()),
                    _ => Err(SnbtError::UnknownKey(compound.object.clone())),
                };
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "nbt" => {
            if first == '\'' || first == '"' {
                if let Some(Content::Resolvable(Resolvable::NBT { path, .. })) =
                    &mut compound.contents[5]
                {
                    *path = Cow::Owned(parse_string(first, chars)?);
                } else {
                    compound.contents[5] = Some(Content::Resolvable(Resolvable::NBT {
                        path: Cow::Owned(parse_string(first, chars)?),
                        interpret: false,
                        plain: false,
                        separator: None,
                        source: NbtSource::Block(Cow::Borrowed("")),
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "interpret" => {
            if let Some(Content::Resolvable(Resolvable::NBT { interpret, .. })) =
                &mut compound.contents[5]
            {
                *interpret = parse_bool(first, chars, "interpret")?;
            } else {
                compound.contents[5] = Some(Content::Resolvable(Resolvable::NBT {
                    path: Cow::Borrowed("-None-"),
                    interpret: parse_bool(first, chars, "interpret")?,
                    plain: false,
                    separator: None,
                    source: NbtSource::Block(Cow::Borrowed("")),
                }));
            }
            Ok(())
        }
        "entity" => {
            if first == '\'' || first == '"' {
                compound.nbt_sources[0] =
                    Some(NbtSource::Entity(Cow::Owned(parse_string(first, chars)?)));
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "block" => {
            if first == '\'' || first == '"' {
                compound.nbt_sources[1] =
                    Some(NbtSource::Block(Cow::Owned(parse_string(first, chars)?)));
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "storage" => {
            if first == '\'' || first == '"' {
                compound.nbt_sources[2] =
                    Some(NbtSource::Storage(Cow::Owned(parse_string(first, chars)?)));
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "object" => {
            if first == '\'' || first == '"' {
                compound.object = parse_string(first, chars)?;
                return match compound.object.as_str() {
                    "player" | "atlas" => Ok(()),
                    _ => Err(SnbtError::UnknownKey(compound.object.clone())),
                };
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "atlas" => {
            if first == '\'' || first == '"' {
                if let Some(Content::Object(Object::Atlas { atlas, .. })) =
                    &mut compound.contents[6]
                {
                    *atlas = Cow::Owned(parse_string(first, chars)?);
                } else {
                    compound.contents[6] = Some(Content::Object(Object::Atlas {
                        atlas: Cow::Owned(parse_string(first, chars)?),
                        sprite: Cow::Borrowed("-None-"),
                        fallback: None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "sprite" => {
            if first == '\'' || first == '"' {
                if let Some(Content::Object(Object::Atlas { sprite, .. })) =
                    &mut compound.contents[6]
                {
                    *sprite = Cow::Owned(parse_string(first, chars)?);
                } else {
                    compound.contents[6] = Some(Content::Object(Object::Atlas {
                        atlas: Cow::Borrowed("minecraft:blocks"),
                        sprite: Cow::Owned(parse_string(first, chars)?),
                        fallback: None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "player" => {
            if first == '{' {
                if let Some(Content::Object(Object::Player { player, .. })) =
                    &mut compound.contents[7]
                {
                    **player = parse_player(chars)?;
                } else {
                    compound.contents[7] = Some(Content::Object(Object::Player {
                        player: Box::new(parse_player(chars)?),
                        hat: true,
                        fallback: None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "hat" => {
            if first == '{' {
                if let Some(Content::Object(Object::Player { hat, .. })) = &mut compound.contents[7]
                {
                    *hat = parse_bool(first, chars, "hat")?;
                } else {
                    compound.contents[7] = Some(Content::Object(Object::Player {
                        player: Box::new(ObjectPlayer {
                            name: None,
                            id: None,
                            texture: None,
                            cape: None,
                            elytra: None,
                            model: None,
                            properties: vec![],
                        }),
                        hat: parse_bool(first, chars, "hat")?,
                        fallback: None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        #[cfg(feature = "custom")]
        "custom" => {
            if first == '{' {
                compound.contents[8] = Some(Content::Custom(parse_custom(chars)?));
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        _ => {
            unknown.add_assign(1);
            Ok(())
        }
    }
}

fn retrieve_content(compound: CompoundParts) -> SnbtResult<Content> {
    let mut error = SnbtError::MissingContent;
    let pos = match compound.content.as_str() {
        "text" => Some(0),
        "translatable" => Some(1),
        "score" => Some(2),
        "selector" => Some(3),
        "keybind" => Some(4),
        "nbt" => Some(5),
        "object" => {
            if compound.object == "player" {
                Some(7)
            } else {
                Some(6)
            }
        }
        #[cfg(feature = "custom")]
        "custom" => Some(8),
        "" => None,
        _ => return Err(SnbtError::UnknownKey(compound.content)),
    };
    for (i, content) in compound.contents.into_iter().enumerate() {
        if let Some(pos) = pos
            && i != pos
        {
            continue;
        }
        if i == 6 && compound.object == "player" {
            continue;
        }
        let Some(content) = content else {
            continue;
        };
        match match_content_type(content, &compound.nbt, &compound.nbt_sources) {
            Ok(content) => return Ok(content),
            Err(err) => error = err,
        }
    }
    Err(error)
}
fn match_content_type(
    mut content: Content,
    nbt: &str,
    nbt_sources: &[Option<NbtSource>; 3],
) -> SnbtResult<Content> {
    match &mut content {
        Content::Translate(msg) => {
            if !msg.key.is_empty() {
                return Ok(content);
            }
            Err(SnbtError::Required(
                String::from("Translations"),
                String::from("key"),
            ))
        }
        Content::Resolvable(Resolvable::Entity { selector, .. }) => {
            if selector != "-None-" {
                return Ok(content);
            }
            Err(SnbtError::Required(
                String::from("Entities"),
                String::from("selector"),
            ))
        }
        Content::Resolvable(Resolvable::NBT { path, source, .. }) => {
            match nbt {
                "entity" => {
                    let Some(entity) = &nbt_sources[0] else {
                        return Err(SnbtError::Required(
                            String::from("Nbt"),
                            String::from("entity"),
                        ));
                    };
                    *source = entity.clone();
                }
                "block" => {
                    let Some(block) = &nbt_sources[1] else {
                        return Err(SnbtError::Required(
                            String::from("Nbt"),
                            String::from("block"),
                        ));
                    };
                    *source = block.clone();
                }
                "storage" => {
                    let Some(storage) = &nbt_sources[2] else {
                        return Err(SnbtError::Required(
                            String::from("Nbt"),
                            String::from("storage"),
                        ));
                    };
                    *source = storage.clone();
                }
                _ => {
                    if let Some(nbt) = nbt_sources.iter().flatten().next() {
                        *source = nbt.clone();
                    }
                }
            }
            if path != "-None-" {
                return Ok(content);
            }
            Err(SnbtError::Required(
                String::from("Nbt"),
                String::from("entity, \"block, or \"storage"),
            ))
        }
        Content::Object(Object::Atlas { sprite, .. }) => {
            if sprite != "-None-" {
                return Ok(content);
            }
            Err(SnbtError::Required(
                String::from("Atlas object"),
                String::from("sprite"),
            ))
        }
        Content::Object(Object::Player { player, .. }) => {
            if !player.is_empty() {
                return Ok(content);
            }
            Err(SnbtError::Required(
                String::from("Player object"),
                String::from("player"),
            ))
        }
        _ => Ok(content),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one match arm per tag/variant; splitting hides the shape"
)]
fn match_format(
    name: &str,
    format: &mut Format,
    first: char,
    chars: &mut Peekable<Chars>,
    unknown: &mut u8,
) -> SnbtResult<()> {
    match name {
        "color" => {
            if first == '\'' || first == '"' {
                let color_str = parse_string(first, chars)?;
                match color_str.as_str() {
                    "aqua" => format.color = Some(Color::Aqua),
                    "black" => format.color = Some(Color::Black),
                    "blue" => format.color = Some(Color::Blue),
                    "dark_aqua" => format.color = Some(Color::DarkAqua),
                    "dark_blue" => format.color = Some(Color::DarkBlue),
                    "dark_gray" => format.color = Some(Color::DarkGray),
                    "dark_green" => format.color = Some(Color::DarkGreen),
                    "dark_purple" => format.color = Some(Color::DarkPurple),
                    "dark_red" => format.color = Some(Color::DarkRed),
                    "gold" => format.color = Some(Color::Gold),
                    "gray" => format.color = Some(Color::Gray),
                    "green" => format.color = Some(Color::Green),
                    "light_purple" => format.color = Some(Color::LightPurple),
                    "red" => format.color = Some(Color::Red),
                    "white" => format.color = Some(Color::White),
                    "yellow" => format.color = Some(Color::Yellow),
                    color => {
                        if let Some(color) = Color::from_hex(color) {
                            format.color = Some(color);
                        } else {
                            return Err(SnbtError::UnknownColor(color.to_string()));
                        }
                    }
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(String::from("color")))
        }
        "font" => {
            if first == '\'' || first == '"' {
                format.font = Some(Cow::Owned(parse_string(first, chars)?));
                return Ok(());
            }
            Err(SnbtError::WrongContentType(String::from("font")))
        }
        "bold" => {
            format.bold = Some(parse_bool(first, chars, "bold")?);
            Ok(())
        }
        "italic" => {
            format.italic = Some(parse_bool(first, chars, "italic")?);
            Ok(())
        }
        "underlined" => {
            format.underlined = Some(parse_bool(first, chars, "underlined")?);
            Ok(())
        }
        "strikethrough" => {
            format.strikethrough = Some(parse_bool(first, chars, "strikethrough")?);
            Ok(())
        }
        "obfuscated" => {
            format.obfuscated = Some(parse_bool(first, chars, "obfuscated")?);
            Ok(())
        }
        "shadow_color" => {
            if first == '[' {
                let mut nums = vec![];
                let mut num = String::new();
                for char in chars.by_ref() {
                    if char == ']' {
                        nums.push(num.clone());
                        break;
                    }
                    if char.is_whitespace() {
                        continue;
                    }
                    if char == ',' {
                        nums.push(num.clone());
                        num = String::new();
                    }
                    if char.is_numeric() || char == '.' {
                        num.push(char);
                    }
                }
                if nums.len() == 4 {
                    let mut nums = nums.iter().enumerate();
                    let mut num = 0_u32;
                    let (_, n) = nums
                        .next_back()
                        .expect("the list was just checked to hold four numbers");
                    let Ok(n) = n.parse::<f32>() else {
                        return Err(SnbtError::WrongContentType(String::from("shadow_color")));
                    };
                    num += ((n * 255.0) as u32) << 24;
                    for (i, n) in nums {
                        let Ok(n) = n.parse::<f32>() else {
                            return Err(SnbtError::WrongContentType(String::from("shadow_color")));
                        };
                        num += ((n * 255.0) as u32) << (16 - 8 * i);
                    }
                    format.shadow_color = Some(num as i32);
                    return Ok(());
                }
                return Err(SnbtError::WrongContentType(String::from("shadow_color")));
            }
            format.shadow_color = Some(parse_num(first, chars, "shadow_color")?.as_i32());
            Ok(())
        }
        _ => {
            unknown.add_assign(1);
            Ok(())
        }
    }
}

fn match_interactions(
    name: &str,
    interactions: &mut Interactivity,
    first: char,
    chars: &mut Peekable<Chars>,
    unknown: &mut u8,
) -> SnbtResult<()> {
    match name {
        "insertion" => {
            if first == '\'' && first == '"' {
                interactions.insertion = Some(Cow::Owned(parse_string(first, chars)?));
                return Ok(());
            }
            Err(SnbtError::WrongContentType(String::from("insertion")))
        }
        "click_event" => {
            if first == '{' {
                interactions.click = Some(MaybeStatic::Owned(Box::new(parse_click(chars)?)));
                return Ok(());
            }
            Err(SnbtError::WrongContentType(String::from("click_event")))
        }
        "hover_event" => {
            if first == '{' {
                interactions.hover = Some(MaybeStatic::Owned(Box::new(parse_hover(chars)?)));
                return Ok(());
            }
            Err(SnbtError::WrongContentType(String::from("hover_event")))
        }
        _ => {
            unknown.add_assign(1);
            Ok(())
        }
    }
}
