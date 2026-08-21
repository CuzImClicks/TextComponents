//! The serde model matches Minecraft's JSON shapes.
#![cfg(feature = "serde")]

use serde_json::json;
use simdnbt::owned::{NbtCompound, NbtTag};
use std::convert::Infallible;
use text_components::interactivity::MaybeStatic;
use text_components::{
    Args, EmbeddedNbtCodec, EncodedNbt, TextComponent,
    content::{NbtSource, Object},
    interactivity::{ClickEvent, Dialog, HoverEvent},
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

static LEFT_ARGS: [TextComponent; 1] = [TextComponent::const_plain("Notch")];

#[test]
fn content_variants_use_minecrafts_json_shapes() {
    let cases = [
        (TextComponent::plain("hello"), json!({"text": "hello"})),
        (
            TextComponent::scoreboard("@s", "points"),
            json!({"score": {"name": "@s", "objective": "points"}}),
        ),
        (TextComponent::entity("@a", None), json!({"selector": "@a"})),
        (
            TextComponent::nbt("Health", NbtSource::Entity("@s".into()), false, None),
            json!({"nbt": "Health", "entity": "@s"}),
        ),
        (
            TextComponent::from(Object::Atlas {
                atlas: "minecraft:blocks".into(),
                sprite: "minecraft:item/diamond".into(),
                fallback: None,
            }),
            json!({"sprite": "minecraft:item/diamond"}),
        ),
        (
            TextComponent::translated(TranslatedMessage::new("disconnect.spam", Args::None)),
            json!({"translate": "disconnect.spam"}),
        ),
        (
            TextComponent::translated(TranslatedMessage::new(
                "disconnect.spam",
                Args::Owned(Box::new([])),
            )),
            json!({"translate": "disconnect.spam", "with": []}),
        ),
        (
            TextComponent::translated(TranslatedMessage {
                fallback: Some("%s left the game".into()),
                ..TranslatedMessage::new("multiplayer.player.left", Args::Static(&LEFT_ARGS))
            }),
            json!({
                "translate": "multiplayer.player.left",
                "fallback": "%s left the game",
                "with": [{"text": "Notch"}],
            }),
        ),
    ];

    for (component, expected) in cases {
        let encoded = serde_json::to_value(&component).expect("component should serialize");
        assert_eq!(encoded, expected);
        let decoded =
            serde_json::from_value::<TextComponent>(encoded).expect("component should deserialize");
        assert_eq!(decoded, component);
    }
}

#[test]
fn interaction_variants_use_minecrafts_json_shapes() {
    let mut patch = NbtCompound::new();
    patch.insert("minecraft:custom_name", "Stone");
    let mut component = TextComponent::plain("events");
    component.interactions.click = Some(MaybeStatic::Owned(Box::new(ClickEvent::ShowDialog {
        dialog: Dialog::Reference("minecraft:test".into()),
    })));
    component.interactions.hover = Some(MaybeStatic::Owned(Box::new(HoverEvent::ShowItem {
        id: "minecraft:stone".into(),
        count: 1,
        components: Some(encoded(NbtTag::Compound(patch))),
    })));
    let expected = json!({
        "text": "events",
        "click_event": {
            "action": "show_dialog",
            "dialog": "minecraft:test"
        },
        "hover_event": {
            "action": "show_item",
            "id": "minecraft:stone",
            "components": {"minecraft:custom_name": "Stone"}
        }
    });

    let encoded = serde_json::to_value(&component).expect("component should serialize");
    assert_eq!(encoded, expected);
    let decoded =
        serde_json::from_value::<TextComponent>(encoded).expect("component should deserialize");
    assert_eq!(decoded, component);
}

#[test]
#[cfg(feature = "custom")]
fn custom_click_payload_is_not_wrapped_in_a_rust_enum_tag() {
    use text_components::custom::{CustomData, Payload};

    let mut payload = NbtCompound::new();
    payload.insert("value", 3_i32);
    let mut component = TextComponent::plain("custom");
    component.interactions.click = Some(MaybeStatic::Owned(Box::new(ClickEvent::Custom(
        CustomData {
            id: "steel:test".into(),
            payload: Payload::Nbt(NbtTag::Compound(payload).into()),
        },
    ))));
    let encoded = serde_json::to_value(component).expect("component should serialize");

    assert_eq!(
        encoded,
        json!({
            "text": "custom",
            "click_event": {
                "action": "custom",
                "id": "steel:test",
                "payload": {"value": 3}
            }
        })
    );
}
