//! Round-trips through the network NBT codec.
#![cfg(feature = "nbt")]

use simdnbt::{
    ToNbtTag,
    owned::{NbtCompound, NbtList, NbtTag},
};
use std::{
    borrow::Cow,
    collections::hash_map::DefaultHasher,
    convert::Infallible,
    hash::{Hash, Hasher},
};
#[cfg(feature = "custom")]
use text_components::custom::{CustomData, Payload};
use text_components::interactivity::{HoverEvent, MaybeStatic};
use text_components::{
    Args, EmbeddedNbtCodec, EncodedNbt, Modifier, NbtValue, TextComponent,
    content::{Content, NbtSource, Object, Resolvable},
    format::Color,
    interactivity::{ClickEvent, Dialog},
    nbt::ComponentDecodeError,
    translation::TranslatedMessage,
};

struct CodecOutput(NbtTag);

impl EmbeddedNbtCodec for CodecOutput {
    type Error = Infallible;

    fn encode_embedded_nbt(self) -> Result<NbtTag, Self::Error> {
        Ok(self.0)
    }
}

fn encoded(value: NbtTag) -> EncodedNbt {
    match EncodedNbt::encode(CodecOutput(value)) {
        Ok(value) => value,
        Err(error) => match error {},
    }
}

fn compound(values: Vec<(&str, NbtTag)>) -> NbtTag {
    NbtTag::Compound(NbtCompound::from_values(
        values
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
    ))
}

#[test]
fn accepts_empty_string_and_non_empty_list_roots() {
    assert_eq!(
        TextComponent::try_from_nbt(&NbtTag::String("".into())),
        Ok(TextComponent::plain(""))
    );

    let list = NbtTag::List(NbtList::String(vec!["first".into(), "second".into()]));
    let component = TextComponent::try_from_nbt(&list).expect("non-empty list should parse");
    assert_eq!(component.content, Content::from(String::from("first")));
    assert_eq!(component.children, vec![TextComponent::plain("second")]);

    assert_eq!(
        TextComponent::try_from_nbt(&NbtTag::List(NbtList::Empty)),
        Err(ComponentDecodeError::EmptyComponentList)
    );
    assert_eq!(
        TextComponent::plain("plain").to_codec_nbt(),
        NbtTag::String("plain".into())
    );
}

#[test]
fn type_discriminator_wins_over_fuzzy_content_matching() {
    let tag = compound(vec![
        ("type", NbtTag::String("selector".into())),
        ("text", NbtTag::String("not selected".into())),
        ("selector", NbtTag::String("@a".into())),
    ]);

    let component = TextComponent::try_from_nbt(&tag).expect("typed selector should parse");
    assert!(matches!(
        component.content,
        Content::Resolvable(Resolvable::Entity { ref selector, .. }) if selector == "@a"
    ));
}

#[test]
fn invalid_nested_components_fail_the_whole_component() {
    let tag = compound(vec![
        ("text", NbtTag::String("parent".into())),
        (
            "extra",
            NbtTag::List(NbtList::Compound(vec![NbtCompound::from_values(vec![(
                "unknown".into(),
                NbtTag::String("value".into()),
            )])])),
        ),
    ]);

    assert!(TextComponent::try_from_nbt(&tag).is_err());
}

#[test]
fn nbt_plain_flag_is_preserved_and_conflicts_with_interpret() {
    let plain_tag = compound(vec![
        ("type", NbtTag::String("nbt".into())),
        ("nbt", NbtTag::String("Health".into())),
        ("plain", NbtTag::Byte(1)),
        ("entity", NbtTag::String("@s".into())),
    ]);
    let component = TextComponent::try_from_nbt(&plain_tag).expect("plain NBT should parse");
    assert!(matches!(
        component.content,
        Content::Resolvable(Resolvable::NBT {
            plain: true,
            source: NbtSource::Entity(ref selector),
            ..
        }) if selector == "@s"
    ));

    let conflicting_tag = compound(vec![
        ("type", NbtTag::String("nbt".into())),
        ("nbt", NbtTag::String("Health".into())),
        ("interpret", NbtTag::Byte(1)),
        ("plain", NbtTag::Byte(1)),
        ("entity", NbtTag::String("@s".into())),
    ]);
    assert_eq!(
        TextComponent::try_from_nbt(&conflicting_tag),
        Err(ComponentDecodeError::ConflictingNbtFlags)
    );
}

#[test]
fn object_fallback_is_parsed_and_serialized() {
    let tag = compound(vec![
        ("type", NbtTag::String("object".into())),
        ("object", NbtTag::String("atlas".into())),
        ("sprite", NbtTag::String("minecraft:item/diamond".into())),
        ("fallback", NbtTag::String("diamond".into())),
    ]);

    let component = TextComponent::try_from_nbt(&tag).expect("object should parse");
    assert!(matches!(
        component.content,
        Content::Object(Object::Atlas {
            ref atlas,
            fallback: Some(ref fallback),
            ..
        }) if atlas == "minecraft:blocks" && **fallback == TextComponent::plain("diamond")
    ));

    let encoded = (&component).to_nbt_tag();
    let decoded = TextComponent::try_from_nbt(&encoded).expect("encoded object should parse");
    assert_eq!(decoded, component);
}

#[test]
fn modern_player_object_fields_round_trip() {
    let tag = compound(vec![
        ("object", NbtTag::String("player".into())),
        (
            "player",
            compound(vec![
                ("name", NbtTag::String("Jeb_".into())),
                ("texture", NbtTag::String("minecraft:skins/jeb".into())),
                ("cape", NbtTag::String("minecraft:capes/test".into())),
                ("elytra", NbtTag::String("minecraft:elytra/test".into())),
                ("model", NbtTag::String("slim".into())),
            ]),
        ),
        ("hat", NbtTag::Byte(0)),
    ]);

    let component = TextComponent::try_from_nbt(&tag).expect("player object should parse");
    let encoded = (&component).to_nbt_tag();
    assert_eq!(TextComponent::try_from_nbt(&encoded), Ok(component));
}

#[test]
#[cfg(feature = "custom")]
fn arbitrary_event_payloads_round_trip_without_losing_nbt_types() {
    let payload = compound(vec![
        ("byte", NbtTag::Byte(4)),
        ("long", NbtTag::Long(9)),
        ("nested", compound(vec![("float", NbtTag::Float(1.25))])),
    ]);
    let component = TextComponent::plain("events")
        .click_event(ClickEvent::Custom(CustomData {
            id: "steel:test".into(),
            payload: Payload::Nbt(NbtValue::from(payload.clone())),
        }))
        .hover_event(HoverEvent::ShowItem {
            id: "minecraft:stone".into(),
            count: 2,
            components: Some(encoded(payload)),
        });

    let encoded = (&component).to_nbt_tag();
    let decoded = TextComponent::try_from_nbt(&encoded).expect("events should round trip");
    assert_eq!(decoded, component);
}

#[test]
fn compatibility_parser_remains_available() {
    assert_eq!(
        TextComponent::from_nbt(&NbtTag::String("text".into())),
        Some(TextComponent::plain("text"))
    );
    assert_eq!(TextComponent::from_nbt(&NbtTag::Int(1)), None);
}

#[test]
fn nbt_value_equality_and_hashing_preserve_float_bits() {
    let first = NbtValue::from(NbtTag::Float(f32::from_bits(0x7fc0_0001)));
    let same = NbtValue::from(NbtTag::Float(f32::from_bits(0x7fc0_0001)));
    let different = NbtValue::from(NbtTag::Float(f32::from_bits(0x7fc0_0002)));

    assert_eq!(first, same);
    assert_ne!(first, different);

    let mut first_hash = DefaultHasher::new();
    first.hash(&mut first_hash);
    let mut same_hash = DefaultHasher::new();
    same.hash(&mut same_hash);
    assert_eq!(first_hash.finish(), same_hash.finish());
}

#[test]
fn resolvable_content_round_trips() {
    let components = [
        TextComponent::scoreboard("@s", "points"),
        TextComponent::entity("@a", Some(TextComponent::plain(" | "))),
        TextComponent::nbt(
            "Inventory[0]",
            NbtSource::Storage("steel:test".into()),
            true,
            Some(TextComponent::plain(" / ")),
        ),
    ];

    for component in components {
        let encoded = (&component).to_nbt_tag();
        assert_eq!(
            TextComponent::try_from_nbt(&encoded),
            Ok(component),
            "resolvable content should survive the component codec"
        );
    }
}

#[test]
fn component_codec_collapses_plain_components_recursively() {
    let component = TextComponent::translated(TranslatedMessage {
        key: Cow::Borrowed("test.message"),
        fallback: None,
        args: Args::Owned(Box::new([TextComponent::plain("argument")])),
    })
    .add_child(TextComponent::plain("child"))
    .hover_event(HoverEvent::show_text("hover"));

    let NbtTag::Compound(encoded) = component.to_codec_nbt() else {
        panic!("styled component should encode as a compound");
    };
    assert_eq!(
        encoded.get("with"),
        Some(&NbtTag::List(NbtList::String(vec!["argument".into()])))
    );
    assert_eq!(
        encoded.get("extra"),
        Some(&NbtTag::List(NbtList::String(vec!["child".into()])))
    );
    let hover = encoded
        .get("hover_event")
        .and_then(NbtTag::compound)
        .expect("hover event should encode as a compound");
    assert_eq!(hover.get("value"), Some(&NbtTag::String("hover".into())));
}

#[test]
fn nested_hover_components_are_encoded_without_resolving_them() {
    let component = TextComponent::plain("hover")
        .hover_event(HoverEvent::show_text(TextComponent::entity("@a", None)));

    let NbtTag::Compound(component) = component.to_codec_nbt() else {
        panic!("hover component should encode as a compound");
    };
    let value = component
        .get("hover_event")
        .and_then(NbtTag::compound)
        .and_then(|hover| hover.get("value"))
        .and_then(NbtTag::compound)
        .expect("hover value should encode as a component compound");
    assert_eq!(value.get("selector"), Some(&NbtTag::String("@a".into())));
}

#[test]
fn embedded_registry_payloads_are_preserved_verbatim() {
    let payload = compound(vec![(
        "minecraft:custom_name",
        NbtTag::String("Stone".into()),
    )]);
    let component = TextComponent::plain("item").hover_event(HoverEvent::ShowItem {
        id: "minecraft:stone".into(),
        count: 1,
        components: Some(encoded(payload.clone())),
    });

    let NbtTag::Compound(component) = component.to_codec_nbt() else {
        panic!("hover component should encode as a compound");
    };
    let hover = component
        .get("hover_event")
        .and_then(NbtTag::compound)
        .expect("hover event should encode as a compound");
    assert_eq!(hover.get("components"), Some(&payload));
}

#[test]
fn modern_style_and_dialog_fields_round_trip() {
    let inline_dialog = compound(vec![
        ("type", NbtTag::String("minecraft:notice".into())),
        ("title", NbtTag::String("Notice".into())),
    ]);
    let mut component = TextComponent::plain("styled");
    component.format.color = Some(Color::Rgb(0x12, 0x34, 0x56));
    component.format.shadow_color = Some(0x7f12_3456);
    component.interactions.click = Some(MaybeStatic::Owned(Box::new(ClickEvent::ShowDialog {
        dialog: Dialog::Inline(encoded(inline_dialog)),
    })));

    let encoded = (&component).to_nbt_tag();
    assert_eq!(TextComponent::try_from_nbt(&encoded), Ok(component));
}
