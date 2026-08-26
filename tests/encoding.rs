#![cfg(feature = "nbt")]

use text_components::{
    EncodedComponent, Modifier, Style, TextComponent, UnresolvedContent, content::NbtSource,
    format::Color, interactivity::HoverEvent,
};
use text_components_macros::{text, text_nbt};

#[test]
fn encoding_matches_writing_the_tree_directly() {
    let component = TextComponent::plain("hello")
        .color(Color::Gold)
        .bold(true)
        .add_child(TextComponent::plain(" world"));

    let mut expected = Vec::new();
    component.to_codec_nbt().write(&mut expected);

    assert_eq!(component.encode().as_bytes(), expected.as_slice());
}

#[test]
fn encoding_matches_the_macro() {
    const STATIC: EncodedComponent = text_nbt!("<gold><b>hello</b></gold>");

    let runtime = TextComponent::plain("hello")
        .color(Color::Gold)
        .bold(true)
        .encode();

    assert_eq!(runtime.as_bytes(), STATIC.as_bytes());
}

#[test]
fn resolvable_content_is_rejected() {
    let scoreboard = TextComponent::scoreboard("player", "kills");
    assert_eq!(
        scoreboard.try_encode(),
        Err(UnresolvedContent::Scoreboard),
        "a scoreboard value cannot be encoded unresolved"
    );

    let nested = TextComponent::plain("kills: ").add_child(TextComponent::nbt(
        "Health",
        NbtSource::entity("@p"),
        false,
        None,
    ));
    assert_eq!(nested.try_encode(), Err(UnresolvedContent::Nbt));

    let hovered = TextComponent::plain("mob")
        .hover_event(HoverEvent::show_text(TextComponent::entity("@a", None)));
    assert_eq!(hovered.try_encode(), Err(UnresolvedContent::Entity));
}

#[test]
fn plain_content_encodes() {
    assert!(TextComponent::plain("hello").try_encode().is_ok());
}

#[test]
fn const_hover_on_a_runtime_hole_encodes() {
    const PREFIX: &str = "Server";
    let who = TextComponent::plain("Steve");

    let encoded = text_nbt!("<hover:show_text:'{const PREFIX}'>{@who}</hover>");

    let component = encoded.decode().expect("the hover template should decode");
    assert!(matches!(
        component.interactions.hover.as_deref(),
        Some(HoverEvent::ShowText { .. })
    ));
}

#[test]
fn const_splice_hover_encodes_in_a_static_template() {
    const NAME: EncodedComponent = text_nbt!("Server");
    const HOVERED: EncodedComponent = text_nbt!("<hover:show_text:'{@const NAME}'>hi</hover>");

    let component = HOVERED.decode().expect("the hover template should decode");
    assert!(matches!(
        component.interactions.hover.as_deref(),
        Some(HoverEvent::ShowText { .. })
    ));
}

#[test]
fn content_tag_bytes_match_the_builders() {
    use text_components::content::ObjectPlayer;
    use uuid::Uuid;

    assert_eq!(
        text_nbt!("<sprite:'minecraft:items':item/emerald/>").as_bytes(),
        TextComponent::atlas("item/emerald", Some("minecraft:items"))
            .encode()
            .as_bytes()
    );
    assert_eq!(
        text_nbt!("<sprite:item/emerald/>").as_bytes(),
        TextComponent::atlas("item/emerald", None::<&str>)
            .encode()
            .as_bytes()
    );
    assert_eq!(
        text_nbt!("<head:Notch/>").as_bytes(),
        TextComponent::player_head(ObjectPlayer::name("Notch"), true)
            .encode()
            .as_bytes()
    );

    let uuid =
        Uuid::parse_str("1f085b2d-9548-4159-a8c7-f3ccdf0c2054").expect("the literal is a UUID");
    let (high, low) = uuid.as_u64_pair();
    let words = [
        ((high >> 32) & 0xFFFF_FFFF) as i32,
        (high & 0xFFFF_FFFF) as i32,
        ((low >> 32) & 0xFFFF_FFFF) as i32,
        (low & 0xFFFF_FFFF) as i32,
    ];
    assert_eq!(
        text_nbt!("<head:1f085b2d-9548-4159-a8c7-f3ccdf0c2054:false/>").as_bytes(),
        TextComponent::player_head(ObjectPlayer::id(words), false)
            .encode()
            .as_bytes()
    );

    assert_eq!(
        text_nbt!("<hover:show_item:'minecraft:diamond_sword':3>x</hover>").as_bytes(),
        TextComponent::plain("x")
            .hover_event(HoverEvent::show_item(
                "minecraft:diamond_sword",
                Some(3),
                None
            ))
            .encode()
            .as_bytes()
    );
    assert_eq!(
        text_nbt!(
            "<hover:show_entity:'minecraft:pig':1f085b2d-9548-4159-a8c7-f3ccdf0c2054:'Pig'>x</hover>"
        )
        .as_bytes(),
        TextComponent::plain("x")
            .hover_event(HoverEvent::show_entity(
                "minecraft:pig",
                uuid,
                Some("Pig".to_string())
            ))
            .encode()
            .as_bytes()
    );
}

#[test]
fn const_content_tags_decode_to_the_tree() {
    const HEAD: EncodedComponent = text_nbt!("<gold><head:Notch/></gold>");
    const SPRITE: EncodedComponent = text_nbt!("<sprite:'minecraft:items':item/emerald/>");

    assert_eq!(
        HEAD.decode().expect("a head should decode"),
        text!("<gold><head:Notch/></gold>")
    );
    assert_eq!(
        SPRITE.decode().expect("a sprite should decode"),
        text!("<sprite:'minecraft:items':item/emerald/>")
    );
}
