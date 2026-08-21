use super::{SnbtError, SnbtResult};
use std::{iter::Peekable, str::Chars};

pub(super) fn parse_string(opener: char, chars: &mut Peekable<Chars>) -> SnbtResult<String> {
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

pub(super) fn parse_bool(
    first: char,
    chars: &mut Peekable<Chars>,
    content_type: &str,
) -> SnbtResult<bool> {
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
    }
    Err(SnbtError::WrongContentType(content_type.to_string()))
}

pub(super) enum Num {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}
impl Num {
    pub const fn as_i32(&self) -> i32 {
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

pub(super) fn parse_num(
    first: char,
    chars: &mut Peekable<Chars>,
    content_type: &str,
) -> SnbtResult<Num> {
    if !first.is_numeric() && first != '-' && first != '.' {
        return Err(SnbtError::WrongContentType(content_type.to_string()));
    }
    let mut num = String::from(first);
    while let Some(next) = chars.peek() {
        if !next.is_numeric() && next != &'-' && first != '.' {
            match next
                .to_lowercase()
                .last()
                .expect("to_lowercase always yields at least one char")
            {
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

pub(super) fn parse_int_vec(
    chars: &mut Peekable<Chars>,
    content_type: &str,
) -> SnbtResult<Vec<i32>> {
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
                }
            }
            _ => return Err(SnbtError::UnfinishedComponent(line!())),
        }
    }
    Err(SnbtError::EndedAbruptely(line!()))
}
