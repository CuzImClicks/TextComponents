use super::scalar::{parse_int_vec, parse_string};
use super::{SnbtError, SnbtResult};
use crate::content::{Content, ObjectPlayer, PlayerProperties, Resolvable};
#[cfg(feature = "custom")]
use crate::custom::{CustomData, Payload};
use std::{borrow::Cow, iter::Peekable, str::Chars};

pub(super) fn parse_scoreboard(chars: &mut Peekable<Chars>) -> SnbtResult<Content> {
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
                            let next = chars.next().expect("`peek` just returned this char");
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
pub(super) fn parse_player(chars: &mut Peekable<Chars>) -> SnbtResult<ObjectPlayer> {
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
                            let next = chars.next().expect("`peek` just returned this char");
                            match name.as_str() {
                                "name" => {
                                    player.name = Some(Cow::Owned(parse_string(next, chars)?));
                                }
                                "texture" => {
                                    player.texture = Some(Cow::Owned(parse_string(next, chars)?));
                                }
                                key => return Err(SnbtError::UnknownKey(key.to_string())),
                            }
                            name = String::new();
                            break;
                        }
                        '[' => {
                            chars.next().expect("`peek` just returned this char");
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
                                    player.properties = properties;
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
                            let next = chars.next().expect("`peek` just returned this char");
                            match name.as_str() {
                                "name" => property.name = Cow::Owned(parse_string(next, chars)?),
                                "value" => property.value = Cow::Owned(parse_string(next, chars)?),
                                "signature" => {
                                    property.signature =
                                        Some(Cow::Owned(parse_string(next, chars)?));
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
pub(super) fn parse_custom(chars: &mut Peekable<Chars>) -> SnbtResult<CustomData> {
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
                            let next = chars.next().expect("`peek` just returned this char");
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
