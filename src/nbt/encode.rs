#[cfg(feature = "custom")]
use crate::custom::Payload;
use crate::{
    Modifier, Style, TextComponent,
    content::{Content, NbtSource, Object, PlayerModel, Resolvable},
    format::{Color, Format},
    interactivity::{ClickEvent, Dialog, HoverEvent, Interactivity},
    resolving::{BuildTarget, NoResolutor, TextResolutor},
    translation::TranslatedMessage,
};
use simdnbt::{
    FromNbtTag, Mutf8String, ToNbtTag,
    owned::{NbtCompound, NbtList, NbtTag},
};

use super::writers::{write_component_elem, write_mutf8};
use super::{TAG_COMPOUND, TAG_LIST, TAG_STRING};

fn encodes_as_string(component: &TextComponent) -> bool {
    matches!(&component.content, Content::Text { .. })
        && component.children.is_empty()
        && component.format.is_none()
        && component.interactions.is_none()
}

impl TranslatedMessage {
    /// Encodes `{translate, fallback?, with?}` straight into network NBT.
    ///
    /// # Panics
    /// If the message carries more args than an NBT list length can hold.
    #[must_use]
    pub fn encode(&self) -> crate::EncodedComponent {
        let mut buf = Vec::with_capacity(64);
        buf.push(TAG_COMPOUND);
        buf.push(TAG_STRING);
        write_mutf8(&mut buf, "translate");
        write_mutf8(&mut buf, &self.key);
        if let Some(fallback) = &self.fallback {
            buf.push(TAG_STRING);
            write_mutf8(&mut buf, "fallback");
            write_mutf8(&mut buf, fallback);
        }
        if !self.args.is_none() {
            let args = self.args.as_slice();
            buf.push(TAG_LIST);
            write_mutf8(&mut buf, "with");
            let len = i32::try_from(args.len())
                .expect("more translation args than an NBT list length can carry");
            if !args.is_empty() && args.iter().all(encodes_as_string) {
                buf.push(TAG_STRING);
                buf.extend_from_slice(&len.to_be_bytes());
                for arg in args {
                    let Content::Text { text } = &arg.content else {
                        unreachable!("encodes_as_string only matches text content");
                    };
                    write_mutf8(&mut buf, text);
                }
            } else {
                buf.push(TAG_COMPOUND);
                buf.extend_from_slice(&len.to_be_bytes());
                for arg in args {
                    write_component_elem(&mut buf, arg);
                }
            }
        }
        buf.push(0);
        crate::EncodedComponent::from_vec(buf)
    }
}

/// Renders a component as the NBT tag vanilla's component codec expects.
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
    #[must_use]
    pub fn to_codec_nbt(&self) -> NbtTag {
        NbtBuilder.build_component(&NoResolutor, self)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match arm per tag/variant; splitting hides the shape"
    )]
    /// The component vanilla shows for an NBT tag, styled by tag type.
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
            NbtTag::Float(n) => format!("{n:?}")
                .color(Color::Gold)
                .add_child("f".color(Color::Red)),
            NbtTag::Double(n) => format!("{n:?}")
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

impl Content {
    #[expect(
        clippy::too_many_lines,
        reason = "one match arm per tag/variant; splitting hides the shape"
    )]
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
                    compound.push(("fallback".into(), (&**fallback).to_nbt_tag()));
                }
                if !msg.args.is_none() {
                    compound.push((
                        "with".into(),
                        NbtTag::List(NbtList::from(
                            msg.args
                                .iter()
                                .map(|component| target.build_component(resolutor, component))
                                .collect::<Vec<_>>(),
                        )),
                    ));
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
        }
    }
}

impl Format {
    fn to_compound(&self, compound: &mut Vec<(Mutf8String, NbtTag)>) {
        if let Some(color) = &self.color {
            compound.push((
                "color".into(),
                NbtTag::String(color.codec_name().as_ref().into()),
            ));
        }
        if let Some(value) = &self.font {
            compound.push(("font".into(), value.to_nbt_tag()));
        }
        if let Some(value) = self.bold {
            compound.push(("bold".into(), NbtTag::Byte(i8::from(value))));
        }
        if let Some(value) = self.italic {
            compound.push(("italic".into(), NbtTag::Byte(i8::from(value))));
        }
        if let Some(value) = self.underlined {
            compound.push(("underlined".into(), NbtTag::Byte(i8::from(value))));
        }
        if let Some(value) = self.strikethrough {
            compound.push(("strikethrough".into(), NbtTag::Byte(i8::from(value))));
        }
        if let Some(value) = self.obfuscated {
            compound.push(("obfuscated".into(), NbtTag::Byte(i8::from(value))));
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
                    ((uuid.0 >> 32) & 0xFFFF_FFFF) as i32,
                    (uuid.0 & 0xFFFF_FFFF) as i32,
                    ((uuid.1 >> 32) & 0xFFFF_FFFF) as i32,
                    (uuid.1 & 0xFFFF_FFFF) as i32,
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

impl HoverEvent {
    /// Encodes this event through Vanilla's codec, without resolution.
    #[must_use]
    pub fn to_codec_nbt(&self) -> NbtTag {
        self.to_nbt_tag(&NoResolutor)
    }
}

impl ClickEvent {
    /// Encodes this event through Vanilla's codec.
    #[must_use]
    pub fn to_codec_nbt(&self) -> NbtTag {
        self.to_nbt_tag()
    }

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
        }
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
