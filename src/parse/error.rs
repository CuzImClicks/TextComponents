use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// Why SNBT parsing failed.
#[derive(Debug)]
pub enum SnbtError {
    /// The SNBT ran out mid-component; the payload is the parser `line!()` that gave up.
    EndedAbruptely(u32),
    /// A component closed while the parser still expected more; the payload is the parser `line!()` that gave up.
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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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

/// A [Result] carrying an [`SnbtError`].
pub type SnbtResult<T> = Result<T, SnbtError>;
