#[cfg(feature = "custom")]
use crate::custom::{CustomData, Payload};
use crate::{
    Modifier, TextComponent,
    content::{Content, NbtSource, Object, ObjectPlayer, PlayerProperties, Resolvable},
    format::{Color, Format},
    interactivity::{ClickEvent, HoverEvent, Interactivity},
    translation::TranslatedMessage,
};
use std::{borrow::Cow, error::Error, fmt::Display, iter::Peekable, ops::AddAssign, str::Chars};
use uuid::Uuid;

#[cfg(feature = "nbt")]
pub mod nbt;

#[derive(Debug)]
pub enum SnbtError {
    EndedAbruptely(u32),
    UnfinishedComponent(u32),
    WrongContentType(String),
    UnknownKey(String),
    MissingContent,
    UnknownColor(String),
    NumberOverflow(String, String),
    Required(String, String),
}
impl Error for SnbtError {}
impl Display for SnbtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnbtError::EndedAbruptely(i) => {
                write!(
                    f,
                    "The SNBT ended in the middle of the component. (Line: {i})"
                )
            }
            SnbtError::UnfinishedComponent(i) => write!(
                f,
                "A component finished at some point, blocking the parsing. (Line: {i})"
            ),
            SnbtError::WrongContentType(content) => {
                write!(f, "Invalid content for the value of {content}.")
            }
            SnbtError::UnknownKey(key) => write!(f, "The key \"{key}\" is unknown."),
            SnbtError::MissingContent => write!(f, "There's a component without any content."),
            SnbtError::UnknownColor(color) => write!(f, "The color \"{color}\" can't be parsed."),
            SnbtError::NumberOverflow(content, num) => {
                write!(f, "In {content}, the value marked as {num} overflows.")
            }
            SnbtError::Required(content, val) => {
                write!(f, "{content} requires \"{val}\" to work, but it's missing.")
            }
        }
    }
}

pub type SnbtResult<T> = Result<T, SnbtError>;

impl TextComponent {
    pub fn from_snbt(string: &str) -> SnbtResult<TextComponent> {
        parse_body(None, &mut string.chars().peekable())
    }
}

fn parse_body(first: Option<char>, chars: &mut Peekable<Chars>) -> SnbtResult<TextComponent> {
    let char = match first {
        Some(first) => first,
        None => {
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
            };
            first
        }
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

fn parse_string(opener: char, chars: &mut Peekable<Chars>) -> SnbtResult<String> {
    let mut content = String::new();
    while let Some(char) = chars.next() {
        if char == opener {
            return Ok(content);
        }
        if char == '\\'
            && let Some(escaped) = chars.next()
        {
            // TODO: Check escapable characters
            match escaped {
                '"' => content.push('"'),
                '\'' => content.push('\''),
                'n' => content.push('\n'),
                '\\' => content.push('\\'),
                _ => (),
            }
            continue;
        }
        content.push(char);
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
    pub fn new() -> Self {
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
                    children,
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
                        args: None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "fallback" => {
            if first == '\'' || first == '"' {
                if let Some(Content::Translate(msg)) = &mut compound.contents[1] {
                    msg.fallback = Some(Cow::Owned(parse_string(first, chars)?));
                } else {
                    compound.contents[1] = Some(Content::Translate(TranslatedMessage {
                        key: Cow::Borrowed(""),
                        fallback: Some(Cow::Owned(parse_string(first, chars)?)),
                        args: None,
                    }));
                }
                return Ok(());
            }
            Err(SnbtError::WrongContentType(name.to_string()))
        }
        "with" => {
            if first == '[' {
                if let Some(Content::Translate(msg)) = &mut compound.contents[1] {
                    msg.args = Some(parse_vec(chars)?.into_boxed_slice());
                } else {
                    compound.contents[1] = Some(Content::Translate(TranslatedMessage {
                        key: Cow::Borrowed(""),
                        fallback: None,
                        args: Some(parse_vec(chars)?.into_boxed_slice()),
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
                    *player = parse_player(chars)?;
                } else {
                    compound.contents[7] = Some(Content::Object(Object::Player {
                        player: parse_player(chars)?,
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
                        player: ObjectPlayer {
                            name: None,
                            id: None,
                            texture: None,
                            cape: None,
                            elytra: None,
                            model: None,
                            properties: vec![],
                        },
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

fn parse_scoreboard(chars: &mut Peekable<Chars>) -> SnbtResult<Content> {
    let mut selector = None;
    let mut objective = None;
    let mut name = String::new();
    let mut in_name = true;
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            '}' => {
                let Some(selector) = selector else {
                    return Err(SnbtError::Required(
                        String::from("Scoreboards"),
                        String::from("name"),
                    ));
                };
                let Some(objective) = objective else {
                    return Err(SnbtError::Required(
                        String::from("Scoreboards"),
                        String::from("objective"),
                    ));
                };
                return Ok(Content::Resolvable(Resolvable::Scoreboard {
                    selector: Cow::Owned(selector),
                    objective: Cow::Owned(objective),
                }));
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
                while let Some(next) = chars.peek() {
                    if next.is_whitespace() {
                        continue;
                    }
                    match next {
                        '\'' | '"' => {
                            let next = chars.next().unwrap();
                            match name.as_str() {
                                "name" => selector = Some(parse_string(next, chars)?),
                                "objective" => objective = Some(parse_string(next, chars)?),
                                key => return Err(SnbtError::UnknownKey(key.to_string())),
                            }
                        }
                        _ => return Err(SnbtError::UnfinishedComponent(line!())),
                    }
                    name = String::new();
                    break;
                }
            }
            ch if in_name => name.push(ch),
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}
fn parse_player(chars: &mut Peekable<Chars>) -> SnbtResult<ObjectPlayer> {
    let mut player = ObjectPlayer {
        name: None,
        id: None,
        texture: None,
        cape: None,
        elytra: None,
        model: None,
        properties: vec![],
    };
    let mut name = String::new();
    let mut in_name = true;
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            '}' => {
                if player.is_empty() {
                    return Err(SnbtError::Required(
                        String::from("Player object"),
                        String::from("name\", \"id\", \"texture\", or \"properties"),
                    ));
                }
                return Ok(player);
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
                while let Some(next) = chars.peek() {
                    if next.is_whitespace() {
                        continue;
                    }
                    match next {
                        '\'' | '"' => {
                            let next = chars.next().unwrap();
                            match name.as_str() {
                                "name" => {
                                    player.name = Some(Cow::Owned(parse_string(next, chars)?))
                                }
                                "texture" => {
                                    player.texture = Some(Cow::Owned(parse_string(next, chars)?))
                                }
                                key => return Err(SnbtError::UnknownKey(key.to_string())),
                            }
                            name = String::new();
                            break;
                        }
                        '[' => {
                            chars.next().unwrap();
                            match name.as_str() {
                                "id" => {
                                    let nums = parse_int_vec(chars, "Player id")?;
                                    if nums.len() != 4 {
                                        return Err(SnbtError::UnfinishedComponent(line!()));
                                    }
                                    player.id = Some([nums[0], nums[1], nums[2], nums[3]]);
                                }
                                "properties" => {
                                    let mut properties = vec![];
                                    while let Some(char) = chars.next() {
                                        if char.is_whitespace() {
                                            continue;
                                        }
                                        match char {
                                            ']' => break,
                                            ',' => (),
                                            '{' => properties.push(parse_player_property(chars)?),
                                            _ => {
                                                return Err(SnbtError::UnfinishedComponent(
                                                    line!(),
                                                ));
                                            }
                                        }
                                    }
                                    player.properties = properties
                                }
                                key => return Err(SnbtError::UnknownKey(key.to_string())),
                            }
                            name = String::new();
                            break;
                        }
                        _ => return Err(SnbtError::UnfinishedComponent(line!())),
                    }
                }
            }
            ch if in_name => name.push(ch),
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}
fn parse_player_property(chars: &mut Peekable<Chars>) -> SnbtResult<PlayerProperties> {
    let mut property = PlayerProperties {
        name: Cow::Borrowed("-None-"),
        value: Cow::Borrowed("-None-"),
        signature: None,
    };
    let mut name = String::new();
    let mut in_name = true;
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            '}' => {
                if property.name == "-None-" {
                    return Err(SnbtError::Required(
                        String::from("Player property"),
                        String::from("name"),
                    ));
                }
                if property.value == "-None-" {
                    return Err(SnbtError::Required(
                        String::from("Player property"),
                        String::from("value"),
                    ));
                }
                return Ok(property);
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
                while let Some(next) = chars.peek() {
                    if next.is_whitespace() {
                        continue;
                    }
                    match next {
                        '\'' | '"' => {
                            let next = chars.next().unwrap();
                            match name.as_str() {
                                "name" => property.name = Cow::Owned(parse_string(next, chars)?),
                                "value" => property.value = Cow::Owned(parse_string(next, chars)?),
                                "signature" => {
                                    property.signature =
                                        Some(Cow::Owned(parse_string(next, chars)?))
                                }
                                key => return Err(SnbtError::UnknownKey(key.to_string())),
                            }
                            name = String::new();
                            break;
                        }
                        _ => return Err(SnbtError::UnfinishedComponent(line!())),
                    }
                }
            }
            ch if in_name => name.push(ch),
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}
#[cfg(feature = "custom")]
fn parse_custom(chars: &mut Peekable<Chars>) -> SnbtResult<CustomData> {
    let mut id = None;
    let mut name = String::new();
    let mut in_name = true;
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            '}' => {
                let Some(id) = id else {
                    return Err(SnbtError::Required(
                        String::from("Custom"),
                        String::from("id"),
                    ));
                };
                return Ok(CustomData {
                    id: Cow::Owned(id),
                    payload: Payload::Empty,
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
                while let Some(next) = chars.peek() {
                    if next.is_whitespace() {
                        continue;
                    }
                    match next {
                        '\'' | '"' => {
                            let next = chars.next().unwrap();
                            match name.as_str() {
                                "id" => id = Some(parse_string(next, chars)?),
                                key => return Err(SnbtError::UnknownKey(key.to_string())),
                            }
                        }
                        // TODO: Add parsing for payloads
                        _ => return Err(SnbtError::UnfinishedComponent(line!())),
                    }
                    name = String::new();
                    break;
                }
            }
            ch if in_name => name.push(ch),
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
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
                    let (_, n) = nums.next_back().unwrap();
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
                };
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
                interactions.click = Some(parse_click(chars)?);
                return Ok(());
            }
            Err(SnbtError::WrongContentType(String::from("click_event")))
        }
        "hover_event" => {
            if first == '{' {
                interactions.hover = Some(parse_hover(chars)?);
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

fn parse_click(chars: &mut Peekable<Chars>) -> SnbtResult<ClickEvent> {
    let mut action = String::new();
    let mut events = [None, None, None, None, None, None, None, None];
    let mut name = String::new();
    let mut in_name = true;
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            '}' => {
                return match action.as_str() {
                    "open_url" => {
                        if let Some(Some(event)) = events.into_iter().next() {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"open_url\""),
                            String::from("url"),
                        ))
                    }
                    "run_command" => {
                        if let Some(Some(event)) = events.into_iter().nth(2) {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"run_command\""),
                            String::from("command"),
                        ))
                    }
                    "suggest_command" => {
                        if let Some(Some(event)) = events.into_iter().nth(3) {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"suggest_command\""),
                            String::from("command"),
                        ))
                    }
                    "change_page" => {
                        if let Some(Some(event)) = events.into_iter().nth(4) {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"change_page\""),
                            String::from("page"),
                        ))
                    }
                    "copy_to_clipboard" => {
                        if let Some(Some(event)) = events.into_iter().nth(5) {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"copy_to_clipboard\""),
                            String::from("value"),
                        ))
                    }
                    "show_dialog" => {
                        if let Some(Some(event)) = events.into_iter().nth(6) {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"show_dialog\""),
                            String::from("dialog"),
                        ))
                    }
                    #[cfg(feature = "custom")]
                    "custom" => {
                        if let Some(Some(event)) = events.into_iter().nth(7) {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"custom\""),
                            String::from("id"),
                        ))
                    }
                    _ => Err(SnbtError::WrongContentType(String::from("action"))),
                };
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
                while let Some(next) = chars.peek() {
                    if next.is_whitespace() {
                        continue;
                    }
                    match next {
                        '\'' | '"' => {
                            let next = chars.next().unwrap();
                            match name.as_str() {
                                "action" => action = parse_string(next, chars)?,
                                "url" => {
                                    events[0] = Some(ClickEvent::OpenUrl {
                                        url: Cow::Owned(parse_string(next, chars)?),
                                    })
                                }
                                "command" => {
                                    let command: Cow<'static, str> =
                                        Cow::Owned(parse_string(next, chars)?);
                                    events[2] = Some(ClickEvent::RunCommand {
                                        command: command.clone(),
                                    });
                                    events[3] = Some(ClickEvent::SuggestCommand { command })
                                }
                                "page" => {
                                    events[4] = Some(ClickEvent::ChangePage {
                                        page: parse_num(next, chars, "page")?.as_i32(),
                                    })
                                }
                                "value" => {
                                    events[5] = Some(ClickEvent::CopyToClipboard {
                                        value: Cow::Owned(parse_string(next, chars)?),
                                    })
                                }
                                "dialog" => {
                                    events[6] = Some(ClickEvent::ShowDialog {
                                        dialog: crate::interactivity::Dialog::Reference(
                                            Cow::Owned(parse_string(next, chars)?),
                                        ),
                                    })
                                }
                                #[cfg(feature = "custom")]
                                "id" => {
                                    events[7] = Some(ClickEvent::Custom(CustomData {
                                        id: Cow::Owned(parse_string(next, chars)?),
                                        payload: Payload::Empty,
                                    }))
                                }
                                #[cfg(feature = "custom")]
                                "payload" => {
                                    let _ = parse_string(next, chars);
                                }
                                key => return Err(SnbtError::UnknownKey(key.to_string())),
                            }
                        }
                        // TODO: Add parsing for payloads
                        _ => return Err(SnbtError::UnfinishedComponent(line!())),
                    }
                    name = String::new();
                    break;
                }
            }
            ch if in_name => name.push(ch),
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}
fn parse_hover(chars: &mut Peekable<Chars>) -> SnbtResult<HoverEvent> {
    let mut action = String::new();
    let mut events = [None, None, None];
    let mut name = String::new();
    let mut in_name = true;
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            '}' => {
                return match action.as_str() {
                    "show_text" => {
                        if let Some(Some(event)) = events.into_iter().next() {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"show_text\""),
                            String::from("value"),
                        ))
                    }
                    "show_item" => {
                        if let Some(Some(event)) = events.into_iter().nth(1)
                            && let HoverEvent::ShowItem { id, .. } = &event
                            && id != "-None-"
                        {
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"show_item\""),
                            String::from("id"),
                        ))
                    }
                    "show_entity" => {
                        if let Some(Some(event)) = events.into_iter().nth(2)
                            && let HoverEvent::ShowEntity { id, uuid, .. } = &event
                        {
                            if id == "-None-" {
                                return Err(SnbtError::Required(
                                    String::from("\"show_entity\""),
                                    String::from("id"),
                                ));
                            }
                            if *uuid == Uuid::nil() {
                                return Err(SnbtError::Required(
                                    String::from("\"show_entity\""),
                                    String::from("uuid"),
                                ));
                            }
                            return Ok(event);
                        }
                        Err(SnbtError::Required(
                            String::from("\"show_entity\""),
                            String::from("id\", and \"uuid"),
                        ))
                    }
                    _ => Err(SnbtError::WrongContentType(String::from("action"))),
                };
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
                while let Some(next) = chars.next() {
                    if next.is_whitespace() {
                        continue;
                    }
                    match name.as_str() {
                        "action" => action = parse_string(next, chars)?,
                        "value" => {
                            events[0] = Some(HoverEvent::ShowText {
                                value: Box::new(parse_body(Some(next), chars)?),
                            })
                        }
                        "id" => match next {
                            '\'' | '"' => {
                                let new_id: Cow<'static, str> =
                                    Cow::Owned(parse_string(next, chars)?);
                                match &mut events[1] {
                                    Some(HoverEvent::ShowItem { id, .. }) => {
                                        *id = new_id.clone();
                                    }
                                    _ => {
                                        events[1] = Some(HoverEvent::ShowItem {
                                            id: new_id.clone(),
                                            count: 1,
                                            components: None,
                                        })
                                    }
                                }
                                match &mut events[2] {
                                    Some(HoverEvent::ShowEntity { id, .. }) => {
                                        *id = new_id;
                                    }
                                    _ => {
                                        events[2] = Some(HoverEvent::ShowEntity {
                                            name: None,
                                            id: new_id,
                                            uuid: Uuid::nil(),
                                        })
                                    }
                                }
                            }
                            _ => return Err(SnbtError::WrongContentType(String::from("id"))),
                        },
                        "count" => match &mut events[1] {
                            Some(HoverEvent::ShowItem { count, .. }) => {
                                *count = parse_num(next, chars, "id")?.as_i32();
                            }
                            _ => {
                                events[1] = Some(HoverEvent::ShowItem {
                                    id: Cow::Borrowed("-None-"),
                                    count: parse_num(next, chars, "id")?.as_i32(),
                                    components: None,
                                })
                            }
                        },
                        "components" => match next {
                            '\'' | '"' => match &mut events[1] {
                                Some(HoverEvent::ShowItem { components, .. }) => {
                                    *components = Some(crate::NbtValue::from(
                                        simdnbt::owned::NbtTag::String(
                                            parse_string(next, chars)?.into(),
                                        ),
                                    ));
                                }
                                _ => {
                                    events[1] = Some(HoverEvent::ShowItem {
                                        id: Cow::Borrowed("-None-"),
                                        count: 1,
                                        components: Some(crate::NbtValue::from(
                                            simdnbt::owned::NbtTag::String(
                                                parse_string(next, chars)?.into(),
                                            ),
                                        )),
                                    })
                                }
                            },

                            _ => {
                                return Err(SnbtError::WrongContentType(String::from(
                                    "components",
                                )));
                            }
                        },
                        "name" => match &mut events[2] {
                            Some(HoverEvent::ShowEntity { name, .. }) => {
                                *name = Some(Box::new(parse_body(Some(next), chars)?));
                            }
                            _ => {
                                events[2] = Some(HoverEvent::ShowEntity {
                                    name: Some(Box::new(parse_body(Some(next), chars)?)),
                                    id: Cow::Borrowed("-None-"),
                                    uuid: Uuid::nil(),
                                })
                            }
                        },
                        "uuid" => {
                            let new_uuid = match next {
                                '\'' | '"' => {
                                    let Ok(uuid) = Uuid::parse_str(&parse_string(next, chars)?)
                                    else {
                                        return Err(SnbtError::WrongContentType(String::from(
                                            "uuid",
                                        )));
                                    };
                                    uuid
                                }
                                '[' => {
                                    let nums = parse_int_vec(chars, "uuid")?;
                                    if nums.len() != 4 {
                                        return Err(SnbtError::WrongContentType(String::from(
                                            "uuid",
                                        )));
                                    }
                                    Uuid::from_u64_pair(
                                        (((nums[0] as u32) as u64) << 32)
                                            + ((nums[1] as u32) as u64),
                                        (((nums[2] as u32) as u64) << 32)
                                            + ((nums[3] as u32) as u64),
                                    )
                                }
                                _ => return Err(SnbtError::WrongContentType(String::from("uuid"))),
                            };

                            match &mut events[2] {
                                Some(HoverEvent::ShowEntity { uuid, .. }) => {
                                    *uuid = new_uuid;
                                }
                                _ => {
                                    events[2] = Some(HoverEvent::ShowEntity {
                                        name: None,
                                        id: Cow::Borrowed("-None-"),
                                        uuid: new_uuid,
                                    })
                                }
                            }
                        }
                        key => return Err(SnbtError::UnknownKey(key.to_string())),
                    }
                    name = String::new();
                    break;
                }
            }
            ch if in_name => name.push(ch),
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}

fn parse_bool(first: char, chars: &mut Peekable<Chars>, content_type: &str) -> SnbtResult<bool> {
    if first.is_numeric() || first == '-' {
        return match parse_num(first, chars, content_type)? {
            Num::I8(num) => Ok(num != 0),
            _ => Err(SnbtError::WrongContentType(content_type.to_string())),
        };
    }
    match first {
        't' => {
            let mut text = String::from('t');
            while let Some(next) = chars.peek() {
                text.push(*next);
                if text == "true" {
                    let _ = chars.next();
                    return Ok(true);
                }
                if "true".starts_with(&text) {
                    let _ = chars.next();
                    continue;
                }
                return Err(SnbtError::WrongContentType(content_type.to_string()));
            }
        }
        'f' => {
            let mut text = String::from('f');
            while let Some(next) = chars.peek() {
                text.push(*next);
                if text == "false" {
                    let _ = chars.next();
                    return Ok(true);
                }
                if "false".starts_with(&text) {
                    let _ = chars.next();
                    continue;
                }
                return Err(SnbtError::WrongContentType(content_type.to_string()));
            }
        }
        _ => (),
    };
    Err(SnbtError::WrongContentType(content_type.to_string()))
}

enum Num {
    /// Snbt byte
    I8(i8),
    /// Snbt short
    I16(i16),
    /// Snbt int
    I32(i32),
    /// Snbt long
    I64(i64),
    /// Snbt float
    F32(f32),
    /// Snbt double
    F64(f64),
}
impl Num {
    pub fn as_i32(&self) -> i32 {
        match self {
            Num::I8(n) => *n as i32,
            Num::I16(n) => *n as i32,
            Num::I32(n) => *n,
            Num::I64(n) => *n as i32,
            Num::F32(n) => *n as i32,
            Num::F64(n) => *n as i32,
        }
    }
}

fn parse_num(first: char, chars: &mut Peekable<Chars>, content_type: &str) -> SnbtResult<Num> {
    if !first.is_numeric() && first != '-' && first != '.' {
        return Err(SnbtError::WrongContentType(content_type.to_string()));
    }
    let mut num = String::from(first);
    while let Some(next) = chars.peek() {
        if !next.is_numeric() && next != &'-' && first != '.' {
            match next.to_lowercase().last().unwrap() {
                'b' => {
                    let _ = chars.next();
                    let Ok(num) = num.parse::<i8>() else {
                        return Err(SnbtError::NumberOverflow(
                            content_type.to_string(),
                            String::from("byte"),
                        ));
                    };
                    return Ok(Num::I8(num));
                }
                's' => {
                    let _ = chars.next();
                    let Ok(num) = num.parse::<i16>() else {
                        return Err(SnbtError::NumberOverflow(
                            content_type.to_string(),
                            String::from("short"),
                        ));
                    };
                    return Ok(Num::I16(num));
                }
                'l' => {
                    let _ = chars.next();
                    let Ok(num) = num.parse::<i64>() else {
                        return Err(SnbtError::NumberOverflow(
                            content_type.to_string(),
                            String::from("long"),
                        ));
                    };
                    return Ok(Num::I64(num));
                }
                'f' => {
                    let _ = chars.next();
                    let Ok(num) = num.parse::<f32>() else {
                        return Err(SnbtError::NumberOverflow(
                            content_type.to_string(),
                            String::from("float"),
                        ));
                    };
                    return Ok(Num::F32(num));
                }
                'd' => {
                    let _ = chars.next();
                    let Ok(num) = num.parse::<f64>() else {
                        return Err(SnbtError::NumberOverflow(
                            content_type.to_string(),
                            String::from("double"),
                        ));
                    };
                    return Ok(Num::F64(num));
                }
                _ => {
                    if num.contains('.') {
                        let Ok(num) = num.parse::<f64>() else {
                            return Err(SnbtError::NumberOverflow(
                                content_type.to_string(),
                                String::from("double"),
                            ));
                        };
                        return Ok(Num::F64(num));
                    }
                    let Ok(num) = num.parse::<i32>() else {
                        return Err(SnbtError::NumberOverflow(
                            content_type.to_string(),
                            String::from("int"),
                        ));
                    };
                    return Ok(Num::I32(num));
                }
            }
        }
        num.push(*next);
        let _ = chars.next();
    }
    Err(SnbtError::WrongContentType(content_type.to_string()))
}

fn parse_int_vec(chars: &mut Peekable<Chars>, content_type: &str) -> SnbtResult<Vec<i32>> {
    let mut nums = vec![];
    let mut inside = false;
    while let Some(char) = chars.next() {
        if char.is_whitespace() {
            continue;
        }
        match char {
            ']' => return Ok(nums),
            ',' => inside = false,
            char if !inside => {
                let num = parse_num(char, chars, content_type)?;
                match num {
                    Num::I32(n) => nums.push(n),
                    _ => {
                        return Err(SnbtError::Required(
                            content_type.to_string(),
                            String::from("ints"),
                        ));
                    }
                }
            }
            'I' => {
                if let Some(&';') = chars.peek() {
                    chars.next();
                } else {
                    return Err(SnbtError::UnfinishedComponent(line!()));
                };
            }
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}
