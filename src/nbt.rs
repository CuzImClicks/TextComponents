#[cfg(feature = "custom")]
use crate::custom::Payload;
use crate::{
    Modifier, TextComponent,
    content::{Content, NbtSource, Object, PlayerModel, Resolvable},
    format::{Color, Format},
    interactivity::{ClickEvent, Dialog, HoverEvent, Interactivity},
    resolving::{BuildTarget, NoResolutor, TextResolutor},
};
use simdnbt::{
    FromNbtTag, Mutf8String, ToNbtTag,
    owned::{BaseNbt, Nbt, NbtCompound, NbtList, NbtTag},
};
use std::ops::Deref as _;

pub use crate::parse::nbt::ComponentDecodeError;

pub struct NbtBuilder;

impl BuildTarget for NbtBuilder {
    type Result = NbtTag;
    fn build_component<R: TextResolutor + ?Sized>(
        &self,
        resolutor: &R,
        component: &TextComponent,
    ) -> NbtTag {
        if let Content::Text { text } = &component.content
            && component.children.is_empty()
            && component.format.is_none()
            && component.interactions.is_none()
        {
            return NbtTag::String(text.as_ref().into());
        }
        NbtTag::Compound(self.build_compound(resolutor, component))
    }
}

impl NbtBuilder {
    fn build_compound<R: TextResolutor + ?Sized>(
        &self,
        resolutor: &R,
        component: &TextComponent,
    ) -> NbtCompound {
        let mut items = vec![];
        component.content.to_compound(&mut items, self, resolutor);
        component.format.to_compound(&mut items);
        component.interactions.to_compound(resolutor, &mut items);
        if !component.children.is_empty() {
            items.push((
                "extra".into(),
                NbtTag::List(NbtList::from(
                    component
                        .children
                        .iter()
                        .map(|component| self.build_component(resolutor, component))
                        .collect::<Vec<_>>(),
                )),
            ));
        }
        NbtCompound::from_values(items)
    }
}

impl TextComponent {
    /// Encodes this component through Vanilla's recursive component codec.
    pub fn to_codec_nbt(&self) -> NbtTag {
        NbtBuilder.build_component(&NoResolutor, self)
    }

    pub fn nbt_display<T: Into<NbtTag>>(tag: T) -> Self {
        let tag = tag.into();
        match tag {
            NbtTag::Byte(n) => n
                .to_string()
                .color(Color::Gold)
                .add_child("b".color(Color::Red)),
            NbtTag::Short(n) => n
                .to_string()
                .color(Color::Gold)
                .add_child("s".color(Color::Red)),
            NbtTag::Int(n) => n.to_string().color(Color::Gold),
            NbtTag::Long(n) => n
                .to_string()
                .color(Color::Gold)
                .add_child("l".color(Color::Red)),
            NbtTag::Float(n) => format!("{:?}", n)
                .color(Color::Gold)
                .add_child("f".color(Color::Red)),
            NbtTag::Double(n) => format!("{:?}", n)
                .color(Color::Gold)
                .add_child("d".color(Color::Red)),
            NbtTag::ByteArray(items) => {
                let component = "["
                    .color(Color::White)
                    .add_children(vec!["B".color(Color::Red), "; ".into()]);
                let mut children = vec![];
                for (i, n) in items.iter().enumerate() {
                    children.push(
                        n.to_string()
                            .color(Color::Gold)
                            .add_child("b".color(Color::Red)),
                    );
                    if i + 1 != items.len() {
                        children.push(", ".into());
                    }
                }
                children.push(TextComponent::plain("]"));
                component.add_children(children)
            }
            NbtTag::String(string) => {
                "\"".add_children(vec![string.to_string().color(Color::Green), "\"".into()])
            }
            NbtTag::List(nbt_list) => {
                let component = "[".color(Color::White);
                let mut children = vec![];
                for (i, tag) in nbt_list.as_nbt_tags().into_iter().enumerate() {
                    children.push(TextComponent::nbt_display(tag));
                    if i + 1 != nbt_list.as_nbt_tags().len() {
                        children.push(", ".into());
                    }
                }
                children.push(TextComponent::plain("]"));
                component.add_children(children)
            }
            NbtTag::Compound(compound) => {
                let component = "{".color(Color::White);
                let mut children = vec![];
                let len = compound.len();
                for (i, (name, tag)) in compound.into_iter().enumerate() {
                    if !name.is_empty() {
                        children.push(name.to_string().color(Color::Aqua));
                        children.push(": ".into());
                    } else if len == 1 {
                        return TextComponent::nbt_display(tag);
                    }
                    children.push(TextComponent::nbt_display(tag));
                    if i + 1 != len {
                        children.push(", ".into());
                    }
                }
                children.push(TextComponent::plain("}"));
                component.add_children(children)
            }
            NbtTag::IntArray(items) => {
                let component = "["
                    .color(Color::White)
                    .add_children(vec!["I".color(Color::Red), "; ".into()]);
                let mut children = vec![];
                for (i, n) in items.iter().enumerate() {
                    children.push(n.to_string().color(Color::Gold));
                    if i + 1 != items.len() {
                        children.push(", ".into());
                    }
                }
                children.push(TextComponent::plain("]"));
                component.add_children(children)
            }
            NbtTag::LongArray(items) => {
                let component = "["
                    .color(Color::White)
                    .add_children(vec!["L".color(Color::Red), "; ".into()]);
                let mut children = vec![];
                for (i, n) in items.iter().enumerate() {
                    children.push(
                        n.to_string()
                            .color(Color::Gold)
                            .add_child("l".color(Color::Red)),
                    );
                    if i + 1 != items.len() {
                        children.push(", ".into());
                    }
                }
                children.push(TextComponent::plain("]"));
                component.add_children(children)
            }
        }
    }
}

pub trait ToSNBT {
    fn to_snbt(&self) -> String;
}

impl ToSNBT for Nbt {
    fn to_snbt(&self) -> String {
        match self {
            Nbt::Some(base) => base.to_snbt(),
            Nbt::None => String::new(),
        }
    }
}
impl ToSNBT for BaseNbt {
    fn to_snbt(&self) -> String {
        let mut child = String::new();
        if !self.name().is_empty() {
            if self.name().to_str().contains(':') {
                child = format!("\"{}\":", self.name());
            } else {
                child = format!("{}:", self.name());
            }
        }
        child.push_str(&self.deref().to_snbt());
        child
    }
}
impl ToSNBT for NbtCompound {
    fn to_snbt(&self) -> String {
        if self.len() == 1 {
            for (name, tag) in self.iter() {
                if name.is_empty() || name.to_str() == "text" {
                    return tag.to_snbt();
                }
            }
        }
        let mut snbt = vec![];
        for (name, tag) in self.iter() {
            let mut child = String::new();
            if !name.is_empty() {
                if name.to_str().contains(':') {
                    child = format!("\"{}\":", name);
                } else {
                    child = format!("{}:", name);
                }
            }
            child.push_str(&tag.to_snbt());
            snbt.push(child);
        }
        format!("{{{}}}", snbt.join(","))
    }
}
impl ToSNBT for NbtTag {
    fn to_snbt(&self) -> String {
        match self {
            NbtTag::Byte(n) => format!("{n}b"),
            NbtTag::Short(n) => format!("{n}s"),
            NbtTag::Int(n) => n.to_string(),
            NbtTag::Long(n) => format!("{n}l"),
            NbtTag::Float(n) => format!("{:?}f", n),
            NbtTag::Double(n) => format!("{:?}d", n),
            NbtTag::ByteArray(items) => format!(
                "[B;{}]",
                items
                    .iter()
                    .map(|n| format!("{n}b"))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            NbtTag::String(str) => format!(
                "\"{}\"",
                // TODO: Check escapable characters
                str.to_string()
                    .replace('\\', "\\\\")
                    .replace('\n', "\\n")
                    .replace('"', "\\\"")
                    .replace('\'', "\\'")
            ),
            NbtTag::List(items) => format!(
                "[{}]",
                items
                    .as_nbt_tags()
                    .iter()
                    .map(|item| item.to_snbt())
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            NbtTag::Compound(nbt) => nbt.to_snbt(),
            NbtTag::IntArray(items) => format!(
                "[I;{}]",
                items
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            NbtTag::LongArray(items) => format!(
                "[L;{}]",
                items
                    .iter()
                    .map(|n| format!("{n}l"))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
        }
    }
}

impl Content {
    fn to_compound<R: TextResolutor + ?Sized>(
        &self,
        compound: &mut Vec<(Mutf8String, NbtTag)>,
        target: &NbtBuilder,
        resolutor: &R,
    ) {
        match self {
            Content::Text { text } => compound.push(("text".into(), text.to_nbt_tag())),
            Content::Object(Object::Atlas {
                atlas,
                sprite,
                fallback,
            }) => {
                if atlas != "minecraft:blocks" {
                    compound.push(("atlas".into(), atlas.to_nbt_tag()));
                }
                compound.push(("sprite".into(), sprite.to_nbt_tag()));
                if let Some(fallback) = fallback {
                    compound.push((
                        "fallback".into(),
                        target.build_component(resolutor, fallback),
                    ));
                }
            }
            Content::Object(Object::Player {
                player,
                hat,
                fallback,
            }) => {
                let mut inner = vec![];
                if let Some(id) = &player.id {
                    inner.push(("id".into(), NbtTag::IntArray(id.to_vec())));
                }
                if let Some(name) = &player.name {
                    inner.push(("name".into(), name.to_nbt_tag()));
                }
                if let Some(texture) = &player.texture {
                    inner.push(("texture".into(), texture.to_nbt_tag()));
                }
                if let Some(cape) = &player.cape {
                    inner.push(("cape".into(), cape.to_nbt_tag()));
                }
                if let Some(elytra) = &player.elytra {
                    inner.push(("elytra".into(), elytra.to_nbt_tag()));
                }
                if let Some(model) = player.model {
                    inner.push((
                        "model".into(),
                        NbtTag::String(
                            match model {
                                PlayerModel::Slim => "slim",
                                PlayerModel::Wide => "wide",
                            }
                            .into(),
                        ),
                    ));
                }
                if !player.properties.is_empty() {
                    inner.push((
                        "properties".into(),
                        NbtTag::List(NbtList::Compound(
                            player
                                .properties
                                .iter()
                                .map(|property| {
                                    let mut compound = vec![
                                        ("name".into(), property.name.to_nbt_tag()),
                                        ("value".into(), property.value.to_nbt_tag()),
                                    ];
                                    if let Some(signature) = &property.signature {
                                        compound.push(("signature".into(), signature.to_nbt_tag()));
                                    }
                                    NbtCompound::from_values(compound)
                                })
                                .collect(),
                        )),
                    ));
                }
                compound.push((
                    "player".into(),
                    NbtTag::Compound(NbtCompound::from_values(inner)),
                ));
                if !hat {
                    compound.push(("hat".into(), NbtTag::Byte(0)));
                }
                if let Some(fallback) = fallback {
                    compound.push((
                        "fallback".into(),
                        target.build_component(resolutor, fallback),
                    ));
                }
            }
            Content::Keybind { keybind } => compound.push(("keybind".into(), keybind.to_nbt_tag())),
            Content::Translate(msg) => {
                compound.push(("translate".into(), msg.key.to_nbt_tag()));
                if let Some(fallback) = &msg.fallback {
                    compound.push(("fallback".into(), fallback.to_nbt_tag()));
                }
                if let Some(args) = &msg.args {
                    compound.push((
                        "with".into(),
                        NbtTag::List(NbtList::from(
                            args.iter()
                                .map(|component| target.build_component(resolutor, component))
                                .collect::<Vec<_>>(),
                        )),
                    ))
                }
            }
            Content::Resolvable(Resolvable::Scoreboard {
                selector,
                objective,
            }) => {
                compound.push((
                    "score".into(),
                    NbtTag::Compound(NbtCompound::from_values(vec![
                        ("name".into(), selector.to_nbt_tag()),
                        ("objective".into(), objective.to_nbt_tag()),
                    ])),
                ));
            }
            Content::Resolvable(Resolvable::Entity {
                selector,
                separator,
            }) => {
                compound.push(("selector".into(), selector.to_nbt_tag()));
                if let Some(separator) = separator {
                    compound.push((
                        "separator".into(),
                        target.build_component(resolutor, separator),
                    ));
                }
            }
            Content::Resolvable(Resolvable::NBT {
                path,
                interpret,
                plain,
                separator,
                source,
            }) => {
                compound.push(("nbt".into(), path.to_nbt_tag()));
                if *interpret {
                    compound.push(("interpret".into(), NbtTag::Byte(1)));
                }
                if *plain {
                    compound.push(("plain".into(), NbtTag::Byte(1)));
                }
                if let Some(separator) = separator {
                    compound.push((
                        "separator".into(),
                        target.build_component(resolutor, separator),
                    ));
                }
                let (field, value) = match source {
                    NbtSource::Entity(value) => ("entity", value),
                    NbtSource::Block(value) => ("block", value),
                    NbtSource::Storage(value) => ("storage", value),
                };
                compound.push((field.into(), value.to_nbt_tag()));
            }
            #[cfg(feature = "custom")]
            Content::Custom(data) => {
                let mut custom = vec![("id".into(), data.id.to_nbt_tag())];
                if let Payload::Nbt(payload) = &data.payload {
                    custom.push(("payload".into(), payload.to_nbt_tag()));
                }
                compound.push((
                    "custom".into(),
                    NbtTag::Compound(NbtCompound::from_values(custom)),
                ));
            }
        };
    }
}

impl Format {
    fn to_compound(&self, compound: &mut Vec<(Mutf8String, NbtTag)>) {
        if let Some(color) = &self.color {
            compound.push((
                "color".into(),
                match color {
                    Color::Black => NbtTag::String("black".into()),
                    Color::DarkBlue => NbtTag::String("dark_blue".into()),
                    Color::DarkGreen => NbtTag::String("dark_green".into()),
                    Color::DarkAqua => NbtTag::String("dark_aqua".into()),
                    Color::DarkRed => NbtTag::String("dark_red".into()),
                    Color::DarkPurple => NbtTag::String("dark_purple".into()),
                    Color::Gold => NbtTag::String("gold".into()),
                    Color::Gray => NbtTag::String("gray".into()),
                    Color::DarkGray => NbtTag::String("dark_gray".into()),
                    Color::Blue => NbtTag::String("blue".into()),
                    Color::Green => NbtTag::String("green".into()),
                    Color::Aqua => NbtTag::String("aqua".into()),
                    Color::Red => NbtTag::String("red".into()),
                    Color::LightPurple => NbtTag::String("light_purple".into()),
                    Color::Yellow => NbtTag::String("yellow".into()),
                    Color::White => NbtTag::String("white".into()),
                    Color::Rgb(r, g, b) => NbtTag::String(format!("#{r:02X}{g:02X}{b:02X}").into()),
                },
            ));
        }
        if let Some(value) = &self.font {
            compound.push(("font".into(), value.to_nbt_tag()));
        }
        if let Some(value) = self.bold {
            compound.push(("bold".into(), NbtTag::Byte(value as i8)));
        }
        if let Some(value) = self.italic {
            compound.push(("italic".into(), NbtTag::Byte(value as i8)));
        }
        if let Some(value) = self.underlined {
            compound.push(("underlined".into(), NbtTag::Byte(value as i8)));
        }
        if let Some(value) = self.strikethrough {
            compound.push(("strikethrough".into(), NbtTag::Byte(value as i8)));
        }
        if let Some(value) = self.obfuscated {
            compound.push(("obfuscated".into(), NbtTag::Byte(value as i8)));
        }
        if let Some(color) = self.shadow_color {
            compound.push(("shadow_color".into(), NbtTag::Int(color)));
        }
    }
}

impl Interactivity {
    fn to_compound<R: TextResolutor + ?Sized>(
        &self,
        resolutor: &R,
        compound: &mut Vec<(Mutf8String, NbtTag)>,
    ) {
        if let Some(insertion) = &self.insertion {
            compound.push((
                "insertion".into(),
                NbtTag::String(insertion.to_string().into()),
            ));
        }
        if let Some(hover) = &self.hover {
            compound.push(("hover_event".into(), hover.to_nbt_tag(resolutor)));
        }
        if let Some(click) = &self.click {
            compound.push(("click_event".into(), click.to_nbt_tag()));
        }
    }
}

impl HoverEvent {
    fn to_nbt_tag<R: TextResolutor + ?Sized>(&self, resolutor: &R) -> NbtTag {
        match self {
            HoverEvent::ShowText { value } => NbtTag::Compound(NbtCompound::from_values(vec![
                ("action".into(), NbtTag::String("show_text".into())),
                ("value".into(), NbtBuilder.build_component(resolutor, value)),
            ])),
            HoverEvent::ShowItem {
                id,
                count,
                components,
            } => {
                let mut compound = vec![
                    ("action".into(), NbtTag::String("show_item".into())),
                    ("id".into(), id.to_nbt_tag()),
                ];
                if *count != 1 {
                    compound.push(("count".into(), NbtTag::Int(*count)));
                }
                if let Some(components) = components
                    && !components.is_empty_compound()
                {
                    compound.push(("components".into(), components.as_nbt().clone()));
                }
                NbtTag::Compound(NbtCompound::from_values(compound))
            }
            HoverEvent::ShowEntity { name, id, uuid } => {
                let uuid = uuid.as_u64_pair();
                let uuid = vec![
                    ((uuid.0 >> 32) & 0xFFFFFFFF) as i32,
                    (uuid.0 & 0xFFFFFFFF) as i32,
                    ((uuid.1 >> 32) & 0xFFFFFFFF) as i32,
                    (uuid.1 & 0xFFFFFFFF) as i32,
                ];
                let mut compound = vec![
                    ("action".into(), NbtTag::String("show_entity".into())),
                    ("id".into(), id.to_nbt_tag()),
                    ("uuid".into(), NbtTag::IntArray(uuid)),
                ];
                if let Some(name) = name {
                    compound.push(("name".into(), NbtBuilder.build_component(resolutor, name)));
                }
                NbtTag::Compound(NbtCompound::from_values(compound))
            }
        }
    }
}

impl ClickEvent {
    fn to_nbt_tag(&self) -> NbtTag {
        let mut values = vec![];
        match &self {
            ClickEvent::OpenUrl { url } => {
                values.push(("action".into(), "open_url".into()));
                values.push(("url".into(), url.to_nbt_tag()));
            }
            ClickEvent::RunCommand { command } => {
                values.push(("action".into(), "run_command".into()));
                values.push(("command".into(), command.to_nbt_tag()));
            }
            ClickEvent::SuggestCommand { command } => {
                values.push(("action".into(), "suggest_command".into()));
                values.push(("command".into(), command.to_nbt_tag()));
            }
            ClickEvent::ChangePage { page } => {
                values.push(("action".into(), "change_page".into()));
                values.push(("page".into(), page.to_nbt_tag()));
            }
            ClickEvent::CopyToClipboard { value } => {
                values.push(("action".into(), "copy_to_clipboard".into()));
                values.push(("value".into(), value.to_nbt_tag()));
            }
            ClickEvent::ShowDialog { dialog } => {
                values.push(("action".into(), "show_dialog".into()));
                values.push((
                    "dialog".into(),
                    match dialog {
                        Dialog::Reference(reference) => reference.to_nbt_tag(),
                        Dialog::Inline(value) => value.as_nbt().clone(),
                    },
                ));
            }
            #[cfg(feature = "custom")]
            ClickEvent::Custom(data) => {
                values.push(("action".into(), "custom".into()));
                values.push(("id".into(), data.id.to_nbt_tag()));
                if let Payload::Nbt(payload) = &data.payload {
                    values.push(("payload".into(), payload.to_nbt_tag()));
                }
            }
        };
        NbtTag::Compound(NbtCompound::from_values(values))
    }
}

impl ToNbtTag for TextComponent {
    fn to_nbt_tag(self) -> NbtTag {
        self.to_codec_nbt()
    }
}
impl ToNbtTag for &TextComponent {
    fn to_nbt_tag(self) -> NbtTag {
        self.to_codec_nbt()
    }
}
impl FromNbtTag for TextComponent {
    fn from_nbt_tag(tag: simdnbt::borrow::NbtTag) -> Option<Self> {
        TextComponent::from_nbt(&tag.to_owned())
    }
}
