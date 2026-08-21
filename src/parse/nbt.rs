use std::{
    borrow::Cow,
    error::Error,
    fmt::{self, Display, Formatter},
};

#[cfg(feature = "custom")]
use crate::custom::{CustomData, Payload};
use crate::{
    EncodedNbt, TextComponent,
    content::{
        Content, NbtSource, Object, ObjectPlayer, PlayerModel, PlayerProperties, Resolvable,
    },
    format::{Color, Format},
    interactivity::{ClickEvent, Dialog, HoverEvent, Interactivity, MaybeStatic},
    translation::{Args, TranslatedMessage},
};
use simdnbt::owned::{NbtCompound, NbtList, NbtTag};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComponentDecodeError {
    ExpectedComponent,
    EmptyComponentList,
    MissingField(&'static str),
    InvalidField {
        field: &'static str,
        expected: &'static str,
    },
    UnknownContentType(String),
    UnknownObjectType(String),
    UnknownDataSource(String),
    UnknownClickAction(String),
    UnknownHoverAction(String),
    NoMatchingContent,
    ConflictingNbtFlags,
}

impl Display for ComponentDecodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedComponent => formatter.write_str("expected a text component"),
            Self::EmptyComponentList => formatter.write_str("component lists cannot be empty"),
            Self::MissingField(field) => write!(formatter, "missing required field `{field}`"),
            Self::InvalidField { field, expected } => {
                write!(formatter, "field `{field}` must be {expected}")
            }
            Self::UnknownContentType(value) => {
                write!(formatter, "unknown component type `{value}`")
            }
            Self::UnknownObjectType(value) => write!(formatter, "unknown object type `{value}`"),
            Self::UnknownDataSource(value) => write!(formatter, "unknown NBT source `{value}`"),
            Self::UnknownClickAction(value) => write!(formatter, "unknown click action `{value}`"),
            Self::UnknownHoverAction(value) => write!(formatter, "unknown hover action `{value}`"),
            Self::NoMatchingContent => formatter.write_str("no matching component content"),
            Self::ConflictingNbtFlags => {
                formatter.write_str("`interpret` and `plain` cannot both be enabled")
            }
        }
    }
}

impl Error for ComponentDecodeError {}

/// A failure turning [`EncodedComponent`](crate::EncodedComponent) bytes back into a tree.
#[derive(Debug)]
pub enum DecodeError {
    /// The bytes were not well-formed NBT.
    Nbt(simdnbt::Error),
    Component(ComponentDecodeError),
}

impl From<simdnbt::Error> for DecodeError {
    fn from(error: simdnbt::Error) -> Self {
        Self::Nbt(error)
    }
}

impl From<ComponentDecodeError> for DecodeError {
    fn from(error: ComponentDecodeError) -> Self {
        Self::Component(error)
    }
}

impl Display for DecodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nbt(error) => write!(formatter, "malformed NBT: {error}"),
            Self::Component(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for DecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Nbt(error) => Some(error),
            Self::Component(error) => Some(error),
        }
    }
}

impl TextComponent {
    /// Reads a component out of a decoded NBT tag.
    pub fn try_from_nbt(tag: &NbtTag) -> Result<Self, ComponentDecodeError> {
        match tag {
            NbtTag::String(value) => Ok(Self::plain(value.to_string())),
            NbtTag::List(list) => component_from_list(list),
            NbtTag::Compound(compound) => component_from_compound(compound),
            _ => Err(ComponentDecodeError::ExpectedComponent),
        }
    }

    /// Compatibility wrapper for callers that do not need parse diagnostics.
    #[must_use]
    pub fn from_nbt(tag: &NbtTag) -> Option<Self> {
        Self::try_from_nbt(tag).ok()
    }
}

fn component_from_list(list: &NbtList) -> Result<TextComponent, ComponentDecodeError> {
    let mut values = list.as_nbt_tags().into_iter();
    let Some(first) = values.next() else {
        return Err(ComponentDecodeError::EmptyComponentList);
    };
    let mut component = TextComponent::try_from_nbt(&first)?;
    for value in values {
        component
            .children
            .to_mut()
            .push(TextComponent::try_from_nbt(&value)?);
    }
    Ok(component)
}

fn component_from_compound(compound: &NbtCompound) -> Result<TextComponent, ComponentDecodeError> {
    if compound.len() == 1
        && let Some(value) = compound.get("")
    {
        return TextComponent::try_from_nbt(value);
    }

    let content = Content::try_from_compound(compound)?;
    let children = match compound.get("extra") {
        None => Vec::new(),
        Some(NbtTag::List(list)) => {
            let values = list.as_nbt_tags();
            if values.is_empty() {
                return Err(ComponentDecodeError::EmptyComponentList);
            }
            values
                .iter()
                .map(TextComponent::try_from_nbt)
                .collect::<Result<Vec<_>, _>>()?
        }
        Some(_) => return Err(invalid("extra", "a non-empty component list")),
    };

    Ok(TextComponent {
        content,
        children: children.into(),
        format: Format::try_from_compound(compound)?,
        interactions: Interactivity::try_from_compound(compound)?,
    })
}

impl Content {
    fn try_from_compound(compound: &NbtCompound) -> Result<Self, ComponentDecodeError> {
        if let Some(content_type) = compound.get("type") {
            let content_type = as_string(content_type)
                .ok_or_else(|| invalid("type", "a component type string"))?;
            return match content_type.as_str() {
                "text" => parse_text(compound),
                "translatable" => parse_translatable(compound),
                "keybind" => parse_keybind(compound),
                "score" => parse_score(compound),
                "selector" => parse_selector(compound),
                "nbt" => parse_nbt(compound),
                "object" => parse_object(compound),
                _ => Err(ComponentDecodeError::UnknownContentType(content_type)),
            };
        }

        if let Ok(content) = parse_text(compound) {
            return Ok(content);
        }
        if let Ok(content) = parse_translatable(compound) {
            return Ok(content);
        }
        if let Ok(content) = parse_keybind(compound) {
            return Ok(content);
        }
        if let Ok(content) = parse_score(compound) {
            return Ok(content);
        }
        if let Ok(content) = parse_selector(compound) {
            return Ok(content);
        }
        if let Ok(content) = parse_nbt(compound) {
            return Ok(content);
        }
        if let Ok(content) = parse_object(compound) {
            return Ok(content);
        }
        #[cfg(feature = "custom")]
        if let Ok(content) = parse_custom_content(compound) {
            return Ok(content);
        }

        Err(ComponentDecodeError::NoMatchingContent)
    }
}

fn parse_text(compound: &NbtCompound) -> Result<Content, ComponentDecodeError> {
    Ok(Content::Text {
        text: required_string(compound, "text")?.into(),
    })
}

fn parse_translatable(compound: &NbtCompound) -> Result<Content, ComponentDecodeError> {
    let key = required_string(compound, "translate")?;
    let fallback = compound
        .get("fallback")
        .and_then(as_string)
        .map(String::into_boxed_str);
    let args = match compound.get("with") {
        None => Args::None,
        Some(NbtTag::List(list)) => Args::Owned(
            list.as_nbt_tags()
                .iter()
                .map(TextComponent::try_from_nbt)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        ),
        Some(_) => return Err(invalid("with", "a component list")),
    };
    Ok(Content::Translate(TranslatedMessage {
        key: key.into(),
        fallback,
        args,
    }))
}

fn parse_keybind(compound: &NbtCompound) -> Result<Content, ComponentDecodeError> {
    Ok(Content::Keybind {
        keybind: required_string(compound, "keybind")?.into(),
    })
}

fn parse_score(compound: &NbtCompound) -> Result<Content, ComponentDecodeError> {
    let score = required_compound(compound, "score")?;
    Ok(Content::Resolvable(Resolvable::Scoreboard {
        selector: required_string(score, "name")?.into(),
        objective: required_string(score, "objective")?.into(),
    }))
}

fn parse_selector(compound: &NbtCompound) -> Result<Content, ComponentDecodeError> {
    let selector = required_string(compound, "selector")?;
    let separator = match compound.get("separator") {
        Some(tag) => Some(Box::new(TextComponent::try_from_nbt(tag)?)),
        None => None,
    };
    Ok(Content::Resolvable(Resolvable::Entity {
        selector: selector.into(),
        separator,
    }))
}

fn parse_nbt(compound: &NbtCompound) -> Result<Content, ComponentDecodeError> {
    let path = required_string(compound, "nbt")?;
    let interpret = compound.get("interpret").and_then(as_bool).unwrap_or(false);
    let plain = compound.get("plain").and_then(as_bool).unwrap_or(false);
    if interpret && plain {
        return Err(ComponentDecodeError::ConflictingNbtFlags);
    }
    let separator = compound
        .get("separator")
        .and_then(|tag| TextComponent::try_from_nbt(tag).ok())
        .map(Box::new);
    Ok(Content::Resolvable(Resolvable::NBT {
        path: path.into(),
        interpret,
        plain,
        separator,
        source: parse_nbt_source(compound)?,
    }))
}

fn parse_nbt_source(compound: &NbtCompound) -> Result<NbtSource, ComponentDecodeError> {
    if let Some(source) = compound.get("source") {
        let source = as_string(source).ok_or_else(|| invalid("source", "a source type string"))?;
        return match source.as_str() {
            "entity" => Ok(NbtSource::Entity(
                required_string(compound, "entity")?.into(),
            )),
            "block" => Ok(NbtSource::Block(required_string(compound, "block")?.into())),
            "storage" => Ok(NbtSource::Storage(
                required_identifier(compound, "storage")?.into(),
            )),
            _ => Err(ComponentDecodeError::UnknownDataSource(source)),
        };
    }

    if let Ok(entity) = required_string(compound, "entity") {
        return Ok(NbtSource::Entity(entity.into()));
    }
    if let Ok(block) = required_string(compound, "block") {
        return Ok(NbtSource::Block(block.into()));
    }
    if let Ok(storage) = required_identifier(compound, "storage") {
        return Ok(NbtSource::Storage(storage.into()));
    }
    Err(ComponentDecodeError::MissingField(
        "entity, block, or storage",
    ))
}

fn parse_object(compound: &NbtCompound) -> Result<Content, ComponentDecodeError> {
    let fallback = match compound.get("fallback") {
        Some(value) => Some(Box::new(TextComponent::try_from_nbt(value)?)),
        None => None,
    };
    let object_type = optional_string(compound, "object")?;
    match object_type.as_deref() {
        Some("atlas") => parse_atlas(compound, fallback),
        Some("player") => parse_player(compound, fallback),
        Some(value) => Err(ComponentDecodeError::UnknownObjectType(value.to_owned())),
        None if compound.contains("sprite") => parse_atlas(compound, fallback),
        None if compound.contains("player") => parse_player(compound, fallback),
        None => Err(ComponentDecodeError::NoMatchingContent),
    }
}

fn parse_atlas(
    compound: &NbtCompound,
    fallback: Option<Box<TextComponent>>,
) -> Result<Content, ComponentDecodeError> {
    let atlas = optional_identifier(compound, "atlas")?
        .map_or(Cow::Borrowed("minecraft:blocks"), Cow::Owned);
    let sprite = required_identifier(compound, "sprite")?;
    Ok(Content::Object(Object::Atlas {
        atlas,
        sprite: sprite.into(),
        fallback,
    }))
}

fn parse_player(
    compound: &NbtCompound,
    fallback: Option<Box<TextComponent>>,
) -> Result<Content, ComponentDecodeError> {
    let player = compound
        .get("player")
        .ok_or(ComponentDecodeError::MissingField("player"))?;
    let player = match player {
        NbtTag::String(name) => {
            let name = name.to_string();
            validate_player_name(&name)?;
            ObjectPlayer::name(name)
        }
        NbtTag::Compound(profile) => parse_player_profile(profile)?,
        _ => return Err(invalid("player", "a player name or profile")),
    };
    let hat = compound.get("hat").and_then(as_bool).unwrap_or(true);
    Ok(Content::Object(Object::Player {
        player: Box::new(player),
        hat,
        fallback,
    }))
}

fn parse_player_profile(profile: &NbtCompound) -> Result<ObjectPlayer, ComponentDecodeError> {
    let name = optional_string(profile, "name")?;
    if let Some(name) = &name {
        validate_player_name(name)?;
    }
    let id = match profile.get("id") {
        None => None,
        Some(NbtTag::IntArray(values) | NbtTag::List(NbtList::Int(values)))
            if values.len() == 4 =>
        {
            Some([values[0], values[1], values[2], values[3]])
        }
        Some(_) => return Err(invalid("id", "a four-integer UUID")),
    };
    let texture = optional_identifier(profile, "texture")?.map(Cow::Owned);
    let cape = optional_identifier(profile, "cape")?.map(Cow::Owned);
    let elytra = optional_identifier(profile, "elytra")?.map(Cow::Owned);
    let model = match optional_string(profile, "model")?.as_deref() {
        None => None,
        Some("slim") => Some(PlayerModel::Slim),
        Some("wide") => Some(PlayerModel::Wide),
        Some(_) => return Err(invalid("model", "`slim` or `wide`")),
    };

    Ok(ObjectPlayer {
        name: name.map(Cow::Owned),
        id,
        texture,
        cape,
        elytra,
        model,
        properties: parse_properties(profile.get("properties"))?,
    })
}

fn parse_properties(tag: Option<&NbtTag>) -> Result<Vec<PlayerProperties>, ComponentDecodeError> {
    let Some(tag) = tag else {
        return Ok(Vec::new());
    };
    let properties = match tag {
        NbtTag::List(NbtList::Compound(properties)) => properties
            .iter()
            .map(|property| {
                Ok(PlayerProperties {
                    name: required_string(property, "name")?.into(),
                    value: required_string(property, "value")?.into(),
                    signature: optional_string(property, "signature")?.map(Cow::Owned),
                })
            })
            .collect::<Result<Vec<_>, ComponentDecodeError>>()?,
        NbtTag::Compound(properties) => {
            let mut result = Vec::new();
            for (name, values) in properties.iter() {
                let NbtTag::List(NbtList::String(values)) = values else {
                    return Err(invalid("properties", "a property map or list"));
                };
                for value in values {
                    result.push(PlayerProperties {
                        name: name.to_string().into(),
                        value: value.to_string().into(),
                        signature: None,
                    });
                }
            }
            result
        }
        _ => return Err(invalid("properties", "a property map or list")),
    };
    if properties.len() > 16 {
        return Err(invalid("properties", "at most 16 properties"));
    }
    Ok(properties)
}

#[cfg(feature = "custom")]
fn parse_custom_content(compound: &NbtCompound) -> Result<Content, ComponentDecodeError> {
    let custom = required_compound(compound, "custom")?;
    Ok(Content::Custom(parse_custom_data(custom)?))
}

impl Format {
    fn try_from_compound(compound: &NbtCompound) -> Result<Self, ComponentDecodeError> {
        let color = match optional_string(compound, "color")? {
            Some(value) => Some(parse_color(&value)?),
            None => None,
        };
        let font = optional_identifier(compound, "font")?.map(Cow::Owned);
        Ok(Self {
            color,
            font,
            bold: optional_bool(compound, "bold")?,
            italic: optional_bool(compound, "italic")?,
            underlined: optional_bool(compound, "underlined")?,
            strikethrough: optional_bool(compound, "strikethrough")?,
            obfuscated: optional_bool(compound, "obfuscated")?,
            shadow_color: match compound.get("shadow_color") {
                Some(tag) => Some(parse_shadow_color(tag)?),
                None => None,
            },
        })
    }
}

impl Interactivity {
    fn try_from_compound(compound: &NbtCompound) -> Result<Self, ComponentDecodeError> {
        Ok(Self {
            insertion: optional_string(compound, "insertion")?.map(Cow::Owned),
            click: match compound.get("click_event") {
                Some(NbtTag::Compound(event)) => Some(MaybeStatic::Owned(Box::new(
                    ClickEvent::try_from_compound(event)?,
                ))),
                Some(_) => return Err(invalid("click_event", "a click event")),
                None => None,
            },
            hover: match compound.get("hover_event") {
                Some(NbtTag::Compound(event)) => Some(MaybeStatic::Owned(Box::new(
                    HoverEvent::try_from_compound(event)?,
                ))),
                Some(_) => return Err(invalid("hover_event", "a hover event")),
                None => None,
            },
        })
    }
}

impl ClickEvent {
    fn try_from_compound(compound: &NbtCompound) -> Result<Self, ComponentDecodeError> {
        let action = required_string(compound, "action")?;
        match action.as_str() {
            "open_url" => {
                let url = required_string(compound, "url")?;
                if !is_allowed_url(&url) {
                    return Err(invalid("url", "an HTTP or HTTPS URI"));
                }
                Ok(Self::OpenUrl { url: url.into() })
            }
            "run_command" => Ok(Self::RunCommand {
                command: required_chat_string(compound, "command")?.into(),
            }),
            "suggest_command" => Ok(Self::SuggestCommand {
                command: required_chat_string(compound, "command")?.into(),
            }),
            "change_page" => {
                let page = required_i32(compound, "page")?;
                if page <= 0 {
                    return Err(invalid("page", "a positive integer"));
                }
                Ok(Self::ChangePage { page })
            }
            "copy_to_clipboard" => Ok(Self::CopyToClipboard {
                value: required_string(compound, "value")?.into(),
            }),
            "show_dialog" => {
                let dialog = compound
                    .get("dialog")
                    .ok_or(ComponentDecodeError::MissingField("dialog"))?;
                let dialog = match dialog {
                    NbtTag::String(value) => {
                        let value = value.to_string();
                        if !is_identifier(&value) {
                            return Err(invalid("dialog", "a dialog identifier or definition"));
                        }
                        Dialog::Reference(value.into())
                    }
                    NbtTag::Compound(_) => {
                        Dialog::Inline(EncodedNbt::from_codec_output(dialog.clone()))
                    }
                    _ => return Err(invalid("dialog", "a dialog identifier or definition")),
                };
                Ok(Self::ShowDialog { dialog })
            }
            #[cfg(feature = "custom")]
            "custom" => Ok(Self::Custom(parse_custom_data(compound)?)),
            _ => Err(ComponentDecodeError::UnknownClickAction(action)),
        }
    }
}

impl HoverEvent {
    fn try_from_compound(compound: &NbtCompound) -> Result<Self, ComponentDecodeError> {
        let action = required_string(compound, "action")?;
        match action.as_str() {
            "show_text" => Ok(Self::ShowText {
                value: MaybeStatic::Owned(Box::new(TextComponent::try_from_nbt(
                    compound
                        .get("value")
                        .ok_or(ComponentDecodeError::MissingField("value"))?,
                )?)),
            }),
            "show_item" => {
                let count = match compound.get("count") {
                    None => 1,
                    Some(tag) => {
                        let count = as_i32(tag).ok_or_else(|| invalid("count", "an integer"))?;
                        if !(1..=99).contains(&count) {
                            return Err(invalid("count", "an integer from 1 through 99"));
                        }
                        count
                    }
                };
                let components = match compound.get("components") {
                    None => None,
                    Some(NbtTag::Compound(components)) if components.is_empty() => None,
                    Some(NbtTag::Compound(components)) => Some(EncodedNbt::from_codec_output(
                        NbtTag::Compound(components.clone()),
                    )),
                    Some(_) => return Err(invalid("components", "a data component patch")),
                };
                Ok(Self::ShowItem {
                    id: required_identifier(compound, "id")?.into(),
                    count,
                    components,
                })
            }
            "show_entity" => {
                let uuid = parse_uuid(
                    compound
                        .get("uuid")
                        .ok_or(ComponentDecodeError::MissingField("uuid"))?,
                )?;
                let name = match compound.get("name") {
                    Some(name) => Some(Box::new(TextComponent::try_from_nbt(name)?)),
                    None => None,
                };
                Ok(Self::ShowEntity {
                    name,
                    id: required_identifier(compound, "id")?.into(),
                    uuid,
                })
            }
            _ => Err(ComponentDecodeError::UnknownHoverAction(action)),
        }
    }
}

#[cfg(feature = "custom")]
fn parse_custom_data(compound: &NbtCompound) -> Result<CustomData, ComponentDecodeError> {
    let payload = compound
        .get("payload")
        .map_or(Payload::Empty, |value| Payload::Nbt(value.clone().into()));
    Ok(CustomData {
        id: required_identifier(compound, "id")?.into(),
        payload,
    })
}

fn parse_uuid(tag: &NbtTag) -> Result<Uuid, ComponentDecodeError> {
    match tag {
        NbtTag::String(value) => {
            Uuid::parse_str(&value.to_string()).map_err(|_| invalid("uuid", "a UUID"))
        }
        NbtTag::IntArray(values) | NbtTag::List(NbtList::Int(values)) if values.len() == 4 => {
            Ok(Uuid::from_u64_pair(
                (u64::from(values[0] as u32) << 32) | u64::from(values[1] as u32),
                (u64::from(values[2] as u32) << 32) | u64::from(values[3] as u32),
            ))
        }
        _ => Err(invalid("uuid", "a UUID")),
    }
}

fn parse_color(value: &str) -> Result<Color, ComponentDecodeError> {
    let color = match value {
        "aqua" => Color::Aqua,
        "black" => Color::Black,
        "blue" => Color::Blue,
        "dark_aqua" => Color::DarkAqua,
        "dark_blue" => Color::DarkBlue,
        "dark_gray" => Color::DarkGray,
        "dark_green" => Color::DarkGreen,
        "dark_purple" => Color::DarkPurple,
        "dark_red" => Color::DarkRed,
        "gold" => Color::Gold,
        "gray" => Color::Gray,
        "green" => Color::Green,
        "light_purple" => Color::LightPurple,
        "red" => Color::Red,
        "white" => Color::White,
        "yellow" => Color::Yellow,
        value => {
            let Some(hex) = value.strip_prefix('#') else {
                return Err(invalid("color", "a text color"));
            };
            let rgb = u32::from_str_radix(hex, 16)
                .ok()
                .filter(|_| !hex.is_empty())
                .filter(|value| *value <= 0x00ff_ffff)
                .ok_or_else(|| invalid("color", "a text color"))?;
            Color::Rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
        }
    };
    Ok(color)
}

fn parse_shadow_color(tag: &NbtTag) -> Result<i32, ComponentDecodeError> {
    if let Some(value) = as_i32(tag) {
        return Ok(value);
    }
    let NbtTag::List(list) = tag else {
        return Err(invalid(
            "shadow_color",
            "an ARGB integer or four-number list",
        ));
    };
    let values = list.as_nbt_tags();
    if values.len() != 4 {
        return Err(invalid(
            "shadow_color",
            "an ARGB integer or four-number list",
        ));
    }
    let mut channels = [0_u32; 4];
    for (channel, value) in channels.iter_mut().zip(values.iter()) {
        let value = as_f32(value)
            .ok_or_else(|| invalid("shadow_color", "an ARGB integer or four-number list"))?;
        *channel = ((value * 255.0).floor() as i32 as u32) & 0xff;
    }
    Ok(((channels[3] << 24) | (channels[0] << 16) | (channels[1] << 8) | channels[2]) as i32)
}

fn required_compound<'a>(
    compound: &'a NbtCompound,
    field: &'static str,
) -> Result<&'a NbtCompound, ComponentDecodeError> {
    match compound.get(field) {
        Some(NbtTag::Compound(value)) => Ok(value),
        Some(_) => Err(invalid(field, "a compound")),
        None => Err(ComponentDecodeError::MissingField(field)),
    }
}

fn required_string(
    compound: &NbtCompound,
    field: &'static str,
) -> Result<String, ComponentDecodeError> {
    match compound.get(field) {
        Some(NbtTag::String(value)) => Ok(value.to_string()),
        Some(_) => Err(invalid(field, "a string")),
        None => Err(ComponentDecodeError::MissingField(field)),
    }
}

fn optional_string(
    compound: &NbtCompound,
    field: &'static str,
) -> Result<Option<String>, ComponentDecodeError> {
    match compound.get(field) {
        Some(NbtTag::String(value)) => Ok(Some(value.to_string())),
        Some(_) => Err(invalid(field, "a string")),
        None => Ok(None),
    }
}

fn required_identifier(
    compound: &NbtCompound,
    field: &'static str,
) -> Result<String, ComponentDecodeError> {
    let value = required_string(compound, field)?;
    if !is_identifier(&value) {
        return Err(invalid(field, "an identifier"));
    }
    Ok(value)
}

fn optional_identifier(
    compound: &NbtCompound,
    field: &'static str,
) -> Result<Option<String>, ComponentDecodeError> {
    let Some(value) = optional_string(compound, field)? else {
        return Ok(None);
    };
    if !is_identifier(&value) {
        return Err(invalid(field, "an identifier"));
    }
    Ok(Some(value))
}

fn required_chat_string(
    compound: &NbtCompound,
    field: &'static str,
) -> Result<String, ComponentDecodeError> {
    let value = required_string(compound, field)?;
    if value
        .chars()
        .any(|character| character == '\u{a7}' || character < ' ' || character == '\u{7f}')
    {
        return Err(invalid(field, "a chat string"));
    }
    Ok(value)
}

fn optional_bool(
    compound: &NbtCompound,
    field: &'static str,
) -> Result<Option<bool>, ComponentDecodeError> {
    match compound.get(field) {
        Some(value) => as_bool(value)
            .map(Some)
            .ok_or_else(|| invalid(field, "a boolean")),
        None => Ok(None),
    }
}

fn required_i32(compound: &NbtCompound, field: &'static str) -> Result<i32, ComponentDecodeError> {
    compound
        .get(field)
        .and_then(as_i32)
        .ok_or_else(|| match compound.get(field) {
            Some(_) => invalid(field, "an integer"),
            None => ComponentDecodeError::MissingField(field),
        })
}

fn as_string(tag: &NbtTag) -> Option<String> {
    match tag {
        NbtTag::String(value) => Some(value.to_string()),
        _ => None,
    }
}

fn as_bool(tag: &NbtTag) -> Option<bool> {
    as_f64(tag).map(|value| value != 0.0)
}

const fn as_i32(tag: &NbtTag) -> Option<i32> {
    match tag {
        NbtTag::Byte(value) => Some(i32::from(*value)),
        NbtTag::Short(value) => Some(i32::from(*value)),
        NbtTag::Int(value) => Some(*value),
        NbtTag::Long(value) => Some(*value as i32),
        NbtTag::Float(value) => Some(*value as i32),
        NbtTag::Double(value) => Some(*value as i32),
        _ => None,
    }
}

fn as_f32(tag: &NbtTag) -> Option<f32> {
    as_f64(tag).map(|value| value as f32)
}

const fn as_f64(tag: &NbtTag) -> Option<f64> {
    match tag {
        NbtTag::Byte(value) => Some(f64::from(*value)),
        NbtTag::Short(value) => Some(f64::from(*value)),
        NbtTag::Int(value) => Some(f64::from(*value)),
        NbtTag::Long(value) => Some(*value as f64),
        NbtTag::Float(value) => Some(f64::from(*value)),
        NbtTag::Double(value) => Some(*value),
        _ => None,
    }
}

fn validate_player_name(value: &str) -> Result<(), ComponentDecodeError> {
    if value.encode_utf16().count() > 16
        || value
            .chars()
            .any(|character| character <= ' ' || character >= '\u{7f}')
    {
        return Err(invalid("name", "a valid player name"));
    }
    Ok(())
}

fn is_identifier(value: &str) -> bool {
    let (namespace, path) = value
        .split_once(':')
        .map_or(("minecraft", value), |(namespace, path)| (namespace, path));
    namespace != ".."
        && namespace.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '-'
                || character == '.'
        })
        && path.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.' | '/')
        })
}

fn is_allowed_url(value: &str) -> bool {
    let Some((scheme, remainder)) = value.split_once(':') else {
        return false;
    };
    matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
        && !remainder.is_empty()
        && !value.chars().any(char::is_whitespace)
}

const fn invalid(field: &'static str, expected: &'static str) -> ComponentDecodeError {
    ComponentDecodeError::InvalidField { field, expected }
}
