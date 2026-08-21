use super::scalar::{parse_int_vec, parse_num, parse_string};
use super::{SnbtError, SnbtResult, parse_body};
#[cfg(feature = "custom")]
use crate::custom::{CustomData, Payload};
use crate::interactivity::{ClickEvent, Dialog, HoverEvent, MaybeStatic};
use simdnbt::owned::NbtTag;
use std::{borrow::Cow, iter::Peekable, str::Chars};
use uuid::Uuid;

#[expect(
    clippy::too_many_lines,
    reason = "one match arm per tag/variant; splitting hides the shape"
)]
pub(super) fn parse_click(chars: &mut Peekable<Chars>) -> SnbtResult<ClickEvent> {
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
                            let next = chars.next().expect("`peek` just returned this char");
                            match name.as_str() {
                                "action" => action = parse_string(next, chars)?,
                                "url" => {
                                    events[0] = Some(ClickEvent::OpenUrl {
                                        url: Cow::Owned(parse_string(next, chars)?),
                                    });
                                }
                                "command" => {
                                    let command: Cow<'static, str> =
                                        Cow::Owned(parse_string(next, chars)?);
                                    events[2] = Some(ClickEvent::RunCommand {
                                        command: command.clone(),
                                    });
                                    events[3] = Some(ClickEvent::SuggestCommand { command });
                                }
                                "page" => {
                                    events[4] = Some(ClickEvent::ChangePage {
                                        page: parse_num(next, chars, "page")?.as_i32(),
                                    });
                                }
                                "value" => {
                                    events[5] = Some(ClickEvent::CopyToClipboard {
                                        value: Cow::Owned(parse_string(next, chars)?),
                                    });
                                }
                                "dialog" => {
                                    events[6] = Some(ClickEvent::ShowDialog {
                                        dialog: Dialog::Reference(Cow::Owned(parse_string(
                                            next, chars,
                                        )?)),
                                    });
                                }
                                #[cfg(feature = "custom")]
                                "id" => {
                                    events[7] = Some(ClickEvent::Custom(CustomData {
                                        id: Cow::Owned(parse_string(next, chars)?),
                                        payload: Payload::Empty,
                                    }));
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
#[expect(
    clippy::too_many_lines,
    reason = "one match arm per tag/variant; splitting hides the shape"
)]
pub(super) fn parse_hover(chars: &mut Peekable<Chars>) -> SnbtResult<HoverEvent> {
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
                                value: MaybeStatic::Owned(Box::new(parse_body(Some(next), chars)?)),
                            });
                        }
                        "id" => match next {
                            '\'' | '"' => {
                                let new_id: Cow<'static, str> =
                                    Cow::Owned(parse_string(next, chars)?);
                                match &mut events[1] {
                                    Some(HoverEvent::ShowItem { id, .. }) => {
                                        id.clone_from(&new_id);
                                    }
                                    _ => {
                                        events[1] = Some(HoverEvent::ShowItem {
                                            id: new_id.clone(),
                                            count: 1,
                                            components: None,
                                        });
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
                                        });
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
                                });
                            }
                        },
                        "components" => match next {
                            '\'' | '"' => match &mut events[1] {
                                Some(HoverEvent::ShowItem { components, .. }) => {
                                    *components = Some(crate::EncodedNbt::from_codec_output(
                                        NbtTag::String(parse_string(next, chars)?.into()),
                                    ));
                                }
                                _ => {
                                    events[1] = Some(HoverEvent::ShowItem {
                                        id: Cow::Borrowed("-None-"),
                                        count: 1,
                                        components: Some(crate::EncodedNbt::from_codec_output(
                                            NbtTag::String(parse_string(next, chars)?.into()),
                                        )),
                                    });
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
                                });
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
                                        (u64::from(nums[0] as u32) << 32)
                                            + u64::from(nums[1] as u32),
                                        (u64::from(nums[2] as u32) << 32)
                                            + u64::from(nums[3] as u32),
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
                                    });
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
