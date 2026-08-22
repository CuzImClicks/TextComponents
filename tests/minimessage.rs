#![cfg(feature = "minimessage")]
#![expect(clippy::unwrap_used, reason = "tests unwrap known-good input")]
//! Runtime parser and compile-time macros must agree on one grammar.

use std::cell::Cell;
use text_components::TextComponent;
use text_components::format::Color;
use text_components::minimessage::{MiniMessage, Value};
use text_components_macros::{text, text_nbt};

fn parse(input: &str) -> TextComponent {
    MiniMessage::new(input).unwrap().fill(&[]).unwrap()
}

fn bump(calls: &Cell<usize>, value: &'static str) -> &'static str {
    calls.set(calls.get() + 1);
    value
}

fn bump_flag(calls: &Cell<usize>) -> bool {
    calls.set(calls.get() + 1);
    true
}

#[test]
fn runtime_rejects_format_specs() {
    let err = MiniMessage::new("<red>{x:.2}</red>").unwrap_err();
    assert!(
        err.to_string().contains("format specs"),
        "unexpected error: {err}"
    );
    assert!(MiniMessage::new("<click:run_command:'/tp {x:.1}'>go</click>").is_err());
}

#[test]
fn static_corpus_matches_macro() {
    assert_eq!(
        parse("<gold><b>Steel</b></gold><gray> dev build</gray>"),
        text!("<gold><b>Steel</b></gold><gray> dev build</gray>")
    );
    assert_eq!(
        parse("<obf>xX</obf><gold><b> Steel </b></gold><obf>Xx</obf>"),
        text!("<obf>xX</obf><gold><b> Steel </b></gold><obf>Xx</obf>")
    );
    assert_eq!(
        parse("<gold><b:false>a</b><!i>b</i><#FF8800>c</#FF8800></gold>"),
        text!("<gold><b:false>a</b><!i>b</i><#FF8800>c</#FF8800></gold>")
    );
    assert_eq!(
        parse(
            "<green><hover:show_text:'<yellow>Guild: Steel'>\
             <click:suggest_command:'/guild info Steel'>[STEEL] </click></hover></green>"
        ),
        text!(
            "<green><hover:show_text:'<yellow>Guild: Steel'>\
             <click:suggest_command:'/guild info Steel'>[STEEL] </click></hover></green>"
        )
    );
    assert_eq!(
        parse("<lang:multiplayer.player.left:'Notch'>"),
        text!("<lang:multiplayer.player.left:'Notch'>")
    );
    assert_eq!(
        parse(
            "<yellow><lang:multiplayer.player.joined.renamed:\
             '<aqua><b>Notch</b></aqua>':'Herobrine <i>the second</i>'></yellow>"
        ),
        text!(
            "<yellow><lang:multiplayer.player.joined.renamed:\
             '<aqua><b>Notch</b></aqua>':'Herobrine <i>the second</i>'></yellow>"
        )
    );
    assert_eq!(
        parse("<hover:show_text:'<lang:multiplayer.player.left:\\'Notch\\'>'>hover me</hover>"),
        text!("<hover:show_text:'<lang:multiplayer.player.left:\\'Notch\\'>'>hover me</hover>")
    );
}

#[test]
fn literal_lang_args_are_const() {
    const LEFT: TextComponent = text!("<lang:multiplayer.player.left:'Notch'>");
    static JOINED: TextComponent = text!(
        "<yellow><lang:multiplayer.player.joined.renamed:\
         '<aqua><b>Notch</b></aqua>':'Herobrine'></yellow>"
    );
    const HOVER: TextComponent =
        text!("<hover:show_text:'<lang:multiplayer.player.left:\\'Notch\\'>'>hover me</hover>");
    assert_eq!(
        HOVER,
        parse("<hover:show_text:'<lang:multiplayer.player.left:\\'Notch\\'>'>hover me</hover>")
    );

    assert_eq!(LEFT, parse("<lang:multiplayer.player.left:'Notch'>"));
    assert_eq!(
        JOINED,
        parse(
            "<yellow><lang:multiplayer.player.joined.renamed:\
             '<aqua><b>Notch</b></aqua>':'Herobrine'></yellow>"
        )
    );
}

#[test]
fn args_none_is_not_an_empty_arg_list() {
    use text_components::Args;
    use text_components::translation::Translation;

    static SPAM: Translation<0> = Translation("disconnect.spam");
    static ARGS: [TextComponent; 1] = [TextComponent::const_plain("Notch")];
    const EMPTY: [TextComponent; 0] = [];
    assert!(SPAM.msg().args.is_none());
    assert!(!SPAM.message(EMPTY).args.is_none());
    assert_ne!(SPAM.msg(), SPAM.message(EMPTY));

    assert_eq!(
        Args::Static(&ARGS),
        Args::Owned(Box::new([TextComponent::plain("Notch")]))
    );
    assert_ne!(Args::None, Args::Static(&[]));
    assert_ne!(Args::None, Args::Owned(Box::new([])));
}

#[test]
fn args_conversions_pick_their_storage() {
    use text_components::Args;

    static ARGS: [TextComponent; 1] = [TextComponent::const_plain("Notch")];
    const EMPTY: [TextComponent; 0] = [];
    let boxed: Box<[TextComponent]> = Box::new([TextComponent::plain("Notch")]);
    let owned = Args::Owned(boxed.clone());

    assert!(matches!(
        Args::from([TextComponent::plain("Notch")]),
        Args::Owned(_)
    ));
    assert!(matches!(
        Args::from(vec![TextComponent::plain("Notch")]),
        Args::Owned(_)
    ));
    assert!(matches!(Args::from(boxed.clone()), Args::Owned(_)));
    assert!(matches!(Args::from(&ARGS[..]), Args::Static(_)));

    assert_eq!(Args::from([TextComponent::plain("Notch")]), owned);
    assert_eq!(Args::from(vec![TextComponent::plain("Notch")]), owned);
    assert_eq!(Args::from(boxed), owned);
    assert_eq!(Args::from(&ARGS[..]), owned);

    assert!(!Args::from(EMPTY).is_none());
}

#[test]
fn color_tag_is_another_spelling() {
    assert_eq!(parse("<color:red>hot</color>"), parse("<red>hot</red>"));
    assert_eq!(
        parse("<color:red>hot</color>"),
        text!("<color:red>hot</color>")
    );
    assert_eq!(
        parse("<color:#ff0000>hot</color>"),
        parse("<#ff0000>hot</#ff0000>")
    );
    assert_eq!(
        parse("<color:#ff0000>hot</color>"),
        text!("<color:#ff0000>hot</color>")
    );
}

#[test]
fn quoted_values_take_escaped_quotes() {
    use text_components::content::Content;
    use text_components::interactivity::HoverEvent;

    let filled = parse("<hover:show_text:'don\\'t'>x</hover>");
    assert_eq!(filled, text!("<hover:show_text:'don\\'t'>x</hover>"));

    match filled.interactions.hover.as_deref() {
        Some(HoverEvent::ShowText { value }) => match &value.content {
            Content::Text { text } => assert_eq!(text.as_ref(), "don't"),
            other => panic!("expected text content, got {other:?}"),
        },
        other => panic!("expected show_text, got {other:?}"),
    }
}

#[test]
fn positional_expressions_are_evaluated_once() {
    // a gradient explodes its span into one piece per character
    let calls = Cell::new(0);
    let filled = text!(
        "<gradient:#000000:#FFFFFF>a{}b</gradient>",
        bump(&calls, "Notch")
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(filled.children.len(), 3);

    let flags = Cell::new(0);
    let filled = text!(
        "<gradient:red:blue><obf:{}>abc</obf></gradient>",
        bump_flag(&flags)
    );
    assert_eq!(flags.get(), 1);
    assert!(
        filled
            .children
            .iter()
            .all(|c| c.format.obfuscated == Some(true))
    );
}

#[test]
fn holes_take_references_and_display_values() {
    use text_components::interactivity::ClickEvent;

    let c = Color::Aqua;
    assert_eq!(text!("<{}>hi</{}>", &c), text!("<aqua>hi</aqua>"));

    let filled = text!(
        "<click:run_command:'/tp {}'><insertion:'{}'>go</insertion></click>",
        5u32,
        5u32
    );
    match filled.interactions.click.as_deref() {
        Some(ClickEvent::RunCommand { command }) => assert_eq!(command.as_ref(), "/tp 5"),
        other => panic!("expected run_command, got {other:?}"),
    }
    assert_eq!(filled.interactions.insertion.as_deref(), Some("5"));
}

#[test]
fn holes_move_only_on_their_last_use() {
    struct NoClone(&'static str);
    impl From<NoClone> for TextComponent {
        fn from(value: NoClone) -> Self {
            TextComponent::plain(value.0)
        }
    }

    let once = NoClone("Notch");
    assert_eq!(text!("{@once}"), TextComponent::plain("Notch"));

    let c = text!("<red>Notch</red>");
    assert_eq!(text!("{@c} {c}").children.len(), 3);
    let c = text!("<red>Notch</red>");
    assert_eq!(text!("{c} {@c}").children.len(), 3);

    let x = text!("<gold>G</gold>");
    let first = text_nbt!("{x}{@x}");
    let x = text!("<gold>G</gold>");
    let second = text_nbt!("{@x}{x}");
    assert_ne!(first, second);

    let name = "Notch".to_string();
    let _ = text!("<red>{}</red>", name);
    assert_eq!(name, "Notch");
}

#[test]
fn fill_matches_macro_with_same_values() {
    let template = MiniMessage::new(
        "<red>☠ </red><{c}>{victim}</{c}><gray> was slain by </gray>{@killer}<obf:{magic}>!!</obf>",
    )
    .unwrap();

    let holes: Vec<&str> = template.holes().collect();
    assert_eq!(holes, ["c", "killer", "magic", "victim"]);

    let killer_component = text!("<gold><hover:show_text:'Rank: MVP+'>Herobrine</hover></gold>");
    let filled = template
        .fill(&[
            ("victim", "Notch".into()),
            ("killer", Value::Component(killer_component.clone())),
            ("c", Color::Aqua.into()),
            ("magic", true.into()),
        ])
        .unwrap();

    let c = Color::Aqua;
    let victim = "Notch".to_string();
    let killer = killer_component;
    let magic = true;
    let via_macro = text!(
        "<red>☠ </red><{c}>{victim}</{c}><gray> was slain by </gray>{@killer}<obf:{magic}>!!</obf>"
    );
    assert_eq!(filled, via_macro);
}

#[test]
fn prestyled_component_keeps_its_own_format() {
    let template = MiniMessage::new("<aqua>{@name}</aqua>").unwrap();
    let purple = TextComponent::plain("Notch");
    let purple = {
        use text_components::Style as _;
        purple.color(Color::LightPurple)
    };
    let filled = template
        .fill(&[("name", Value::Component(purple))])
        .unwrap();
    assert_eq!(filled.format.color, Some(Color::LightPurple));
}

#[test]
fn holes_inside_hover_and_click() {
    use text_components::interactivity::ClickEvent;

    let template = MiniMessage::new(
        "<aqua><hover:show_text:'<yellow>Rank: {rank}'>\
         <click:suggest_command:'/msg {name} '>{name}</click></hover></aqua>",
    )
    .unwrap();
    let holes: Vec<&str> = template.holes().collect();
    assert_eq!(holes, ["name", "rank"]);

    let filled = template
        .fill(&[("rank", "MVP+".into()), ("name", "Notch".into())])
        .unwrap();

    let rank = "MVP+".to_string();
    let name = "Notch".to_string();
    let via_macro = text!(
        "<aqua><hover:show_text:'<yellow>Rank: {rank}'>\
         <click:suggest_command:'/msg {name} '>{name}</click></hover></aqua>"
    );
    assert_eq!(filled, via_macro);

    match filled.interactions.click.as_deref() {
        Some(ClickEvent::SuggestCommand { command }) => {
            assert_eq!(command.as_ref(), "/msg Notch ");
        }
        other => panic!("expected suggest_command, got {other:?}"),
    }
}

#[test]
fn event_holes_carry_whole_events() {
    use text_components::interactivity::{ClickEvent, HoverEvent};

    let template =
        MiniMessage::new("<gold><hover:{item}><click:{cmd}>[Sword]</click></hover></gold>")
            .unwrap();
    let holes: Vec<&str> = template.holes().collect();
    assert_eq!(holes, ["cmd", "item"]);

    let sword = HoverEvent::show_item("minecraft:diamond_sword", Some(3), None);
    let give = ClickEvent::run_command("/give @s diamond_sword");
    let filled = template
        .fill(&[("item", sword.clone().into()), ("cmd", give.clone().into())])
        .unwrap();

    let item = sword;
    let cmd = give;
    let via_macro = text!("<gold><hover:{item}><click:{cmd}>[Sword]</click></hover></gold>");
    assert_eq!(filled, via_macro);

    match filled.interactions.hover.as_deref() {
        Some(HoverEvent::ShowItem { id, count, .. }) => {
            assert_eq!(id.as_ref(), "minecraft:diamond_sword");
            assert_eq!(*count, 3);
        }
        other => panic!("expected show_item, got {other:?}"),
    }

    assert_eq!(
        via_macro,
        text!(
            "<gold><hover:{}><click:{}>[Sword]</click></hover></gold>",
            HoverEvent::show_item("minecraft:diamond_sword", Some(3), None),
            ClickEvent::run_command("/give @s diamond_sword")
        )
    );
}

#[test]
fn show_item_by_tag_points_at_the_event_hole() {
    let err = MiniMessage::new("<hover:show_item:'diamond'>x</hover>").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("needs runtime data"), "{msg}");
    assert!(msg.contains("<hover:{item}>"), "{msg}");
}

#[test]
fn keybind_font_shadow_and_insertion() {
    use text_components::content::Content;

    assert_eq!(
        parse("<gray>Press </gray><key:key.jump><gray> to fly</gray>"),
        text!("<gray>Press </gray><key:key.jump><gray> to fly</gray>")
    );
    assert_eq!(
        parse("<font:minecraft:uniform><shadow:#80FF0000>fancy</shadow></font>"),
        text!("<font:minecraft:uniform><shadow:#80FF0000>fancy</shadow></font>")
    );

    let keyed = parse("<key:key.inventory>");
    assert!(
        matches!(&keyed.content, Content::Keybind { keybind } if keybind == "key.inventory"),
        "{keyed:?}"
    );

    let font = parse("<font:minecraft:alt>x</font>");
    assert_eq!(font.format.font.as_deref(), Some("minecraft:alt"));
    let shadow = parse("<shadow:#80FF0000>x</shadow>");
    assert_eq!(shadow.format.shadow_color, Some(0x80FF_0000_u32 as i32));
    // no alpha given means 25%
    assert_eq!(
        parse("<shadow:red>x</shadow>").format.shadow_color,
        Some(0x40FF_5555_u32 as i32)
    );

    let template = MiniMessage::new("<insertion:'/msg {name} '>{name}</insertion>").unwrap();
    let filled = template.fill(&[("name", "Notch".into())]).unwrap();
    assert_eq!(
        filled.interactions.insertion.as_deref(),
        Some("/msg Notch ")
    );

    let name = "Notch".to_string();
    assert_eq!(
        filled,
        text!("<insertion:'/msg {name} '>{name}</insertion>")
    );
}

#[test]
fn shadow_takes_an_alpha() {
    assert_eq!(
        parse("<shadow:red:0.5>x</shadow>").format.shadow_color,
        Some(0x80FF_5555_u32 as i32)
    );
    assert_eq!(
        parse("<shadow:#ff0000:0.5>x</shadow>").format.shadow_color,
        Some(0x80FF_0000_u32 as i32)
    );
    assert_eq!(parse("<!shadow>x").format.shadow_color, Some(0));

    assert_eq!(
        parse("<shadow:red:0.5>x</shadow>"),
        text!("<shadow:red:0.5>x</shadow>")
    );
    assert_eq!(parse("<!shadow>x"), text!("<!shadow>x"));

    let err = MiniMessage::new("<shadow:red:2>x").unwrap_err();
    assert!(
        err.to_string().contains("not an alpha between 0 and 1"),
        "{err}"
    );
}

#[test]
fn insert_is_the_canonical_insertion() {
    let template = MiniMessage::new("<insert:'/msg {name} '>{name}</insert>").unwrap();
    let filled = template.fill(&[("name", "Notch".into())]).unwrap();
    assert_eq!(
        filled.interactions.insertion.as_deref(),
        Some("/msg Notch ")
    );

    let name = "Notch".to_string();
    assert_eq!(filled, text!("<insert:'/msg {name} '>{name}</insert>"));
    assert_eq!(
        parse("<insert:'x'>a</insertion>"),
        parse("<insertion:'x'>a</insertion>")
    );
    assert_eq!(
        text!("<insert:'x'>a</insertion>"),
        text!("<insertion:'x'>a</insert>")
    );
}

#[test]
fn tag_names_ignore_case() {
    assert_eq!(parse("<RED>x</Red>"), parse("<red>x</red>"));
    assert_eq!(parse("<C:blue>x</c>"), parse("<color:blue>x</color>"));
    assert_eq!(parse("<colour:#00ff00>x"), parse("<color:#00ff00>x"));
    assert_eq!(text!("<RED>x</Red>"), text!("<red>x</red>"));
    assert_eq!(text!("<C:blue>x</C>"), text!("<blue>x</blue>"));

    // a hole names a variable, so its case survives
    let template = MiniMessage::new("<{Color}>x</{Color}>").unwrap();
    assert_eq!(template.holes().collect::<Vec<_>>(), ["Color"]);
    let filled = template.fill(&[("Color", Color::Aqua.into())]).unwrap();
    assert_eq!(filled.format.color, Some(Color::Aqua));
}

#[test]
fn arguments_take_either_quote() {
    use text_components::interactivity::ClickEvent;

    assert_eq!(
        parse("<hover:show_text:\"hi\">x</hover>"),
        parse("<hover:show_text:'hi'>x</hover>")
    );
    assert_eq!(
        parse("<hover:show_text:\"hi\">x</hover>"),
        text!("<hover:show_text:\"hi\">x</hover>")
    );
    assert_eq!(
        parse("<hover:show_text:\"it's\">x</hover>"),
        text!("<hover:show_text:\"it's\">x</hover>")
    );

    let filled = parse("<click:run_command:\"/say \\\"x\\\"\">go</click>");
    match filled.interactions.click.as_deref() {
        Some(ClickEvent::RunCommand { command }) => assert_eq!(command.as_ref(), "/say \"x\""),
        other => panic!("expected run_command, got {other:?}"),
    }
    assert_eq!(
        filled,
        text!("<click:run_command:\"/say \\\"x\\\"\">go</click>")
    );

    let err = MiniMessage::new("<hover:show_text:\"a'>x").unwrap_err();
    assert!(err.to_string().contains("unclosed `\"`"), "{err}");
}

#[test]
fn self_closing_tags_take_no_content() {
    assert_eq!(parse("<key:key.jump/>x"), parse("<key:key.jump>x"));
    assert_eq!(text!("<key:key.jump/>x"), text!("<key:key.jump>x"));
    assert_eq!(parse("<red/>x"), TextComponent::plain("x"));
    assert_eq!(text!("<red/>x"), text!("x"));
    assert_eq!(parse("a<newline/>b"), parse("a<br>b"));
    assert_eq!(parse("<font:a/b>x").format.font.as_deref(), Some("a/b"));
}

#[test]
fn lang_or_shows_a_fallback() {
    use text_components::content::Content;

    let filled = parse("<lang_or:my.key:'Fallback text'>");
    match &filled.content {
        Content::Translate(msg) => {
            assert_eq!(msg.key.as_ref(), "my.key");
            assert_eq!(msg.fallback.as_deref(), Some("Fallback text"));
            assert!(msg.args.is_none());
        }
        other => panic!("expected a translation, got {other:?}"),
    }
    assert_eq!(filled, text!("<lang_or:my.key:'Fallback text'>"));

    let template = MiniMessage::new("<tr_or:my.key:'Hi {name}':'{@arg}'>").unwrap();
    assert_eq!(template.holes().collect::<Vec<_>>(), ["arg", "name"]);
    let component = text!("<aqua>Notch</aqua>");
    let filled = template
        .fill(&[
            ("name", "Notch".into()),
            ("arg", Value::Component(component.clone())),
        ])
        .unwrap();

    let name = "Notch".to_string();
    let arg = component;
    assert_eq!(filled, text!("<tr_or:my.key:'Hi {name}':'{@arg}'>"));

    let err = MiniMessage::new("<lang_or:my.key>").unwrap_err();
    assert!(err.to_string().contains("needs a fallback"), "{err}");
}

#[test]
fn click_show_dialog_references_a_dialog() {
    use text_components::interactivity::{ClickEvent, Dialog};

    let filled = parse("<click:show_dialog:'steel:menu'>open</click>");
    match filled.interactions.click.as_deref() {
        Some(ClickEvent::ShowDialog {
            dialog: Dialog::Reference(id),
        }) => assert_eq!(id.as_ref(), "steel:menu"),
        other => panic!("expected show_dialog, got {other:?}"),
    }
    assert_eq!(
        filled,
        text!("<click:show_dialog:'steel:menu'>open</click>")
    );

    let template = MiniMessage::new("<click:show_dialog:'{id}'>open</click>").unwrap();
    let filled = template.fill(&[("id", "steel:menu".into())]).unwrap();
    assert_eq!(
        filled,
        text!("<click:show_dialog:'steel:menu'>open</click>")
    );
}

#[test]
fn gradient_colors_every_character() {
    let filled = parse("<gradient:#000000:#FFFFFF>abcde</gradient>");
    let colors: Vec<Option<Color>> = filled
        .children
        .iter()
        .map(|child| child.format.color.clone())
        .collect();
    assert_eq!(
        colors,
        vec![
            Some(Color::Rgb(0x00, 0x00, 0x00)),
            Some(Color::Rgb(0x40, 0x40, 0x40)),
            Some(Color::Rgb(0x80, 0x80, 0x80)),
            Some(Color::Rgb(0xBF, 0xBF, 0xBF)),
            Some(Color::Rgb(0xFF, 0xFF, 0xFF)),
        ]
    );
    assert_eq!(filled, text!("<gradient:#000000:#FFFFFF>abcde</gradient>"));

    let bold = parse("<gradient:red:blue><b>ab</b></gradient>");
    assert!(bold.children.iter().all(|c| c.format.bold == Some(true)));
    assert_eq!(
        bold.children[0].format.color,
        Some(Color::Rgb(0xFF, 0x55, 0x55))
    );
    assert_eq!(
        bold.children[1].format.color,
        Some(Color::Rgb(0x55, 0x55, 0xFF))
    );

    let mixed = parse("<gradient:#000000:#FFFFFF>a<green>b</green>c</gradient>");
    assert_eq!(mixed.children[1].format.color, Some(Color::Green));
    assert_eq!(
        mixed.children[2].format.color,
        Some(Color::Rgb(0xFF, 0xFF, 0xFF))
    );

    let holed = MiniMessage::new("<gradient:#000000:#FFFFFF>a{name}b</gradient>").unwrap();
    let holed = holed.fill(&[("name", "Notch".into())]).unwrap();
    assert_eq!(holed.children.len(), 3);
    assert_eq!(
        holed.children[1].format.color,
        Some(Color::Rgb(0x80, 0x80, 0x80))
    );

    assert_eq!(
        parse("<gradient:red:blue>ab"),
        parse("<gradient:red:blue>ab</gradient>")
    );

    let rainbow = parse("<rainbow>abc</rainbow>");
    assert_eq!(rainbow.children.len(), 3);
    assert_eq!(
        rainbow.children[0].format.color,
        Some(Color::Rgb(255, 0, 0))
    );
    assert_eq!(
        rainbow.children[1].format.color,
        Some(Color::Rgb(0, 255, 0))
    );
    assert_eq!(
        rainbow.children[2].format.color,
        Some(Color::Rgb(0, 0, 255))
    );
    assert_eq!(rainbow, text!("<rainbow>abc</rainbow>"));
}

#[test]
fn gradient_errors_are_helpful() {
    let err = MiniMessage::new("<gradient:#ff0000>x</gradient>").unwrap_err();
    assert!(
        err.to_string().contains("needs at least two colors"),
        "{err}"
    );
    let err = MiniMessage::new("<gradient:red:mauve>x</gradient>").unwrap_err();
    assert!(err.to_string().contains("`mauve` is not a color"), "{err}");
    let err = MiniMessage::new("<rainbow:red>x</rainbow>").unwrap_err();
    assert!(err.to_string().contains("takes no arguments"), "{err}");
}

#[test]
fn change_page_rejects_holes() {
    let err = MiniMessage::new("<click:change_page:'{page}'>next</click>").unwrap_err();
    assert!(
        err.to_string().contains("change_page can't take holes"),
        "{err}"
    );
}

#[test]
fn parse_errors_are_rich() {
    let err = MiniMessage::new("<red>☠ </red><drak_gray>oops").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("unknown tag `<drak_gray>`"), "{msg}");
    assert!(msg.contains("did you mean `<dark_gray>`?"), "{msg}");
    assert!(msg.contains("^^^^^^^^^^^"), "{msg}");

    let err = MiniMessage::new("<aqua><b>MVP</gold>").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("doesn't match any open tag"), "{msg}");
    assert!(msg.contains("currently open: <aqua>, <bold>"), "{msg}");

    let err = MiniMessage::new("hello {}").unwrap_err();
    assert!(
        err.to_string()
            .contains("runtime templates need named holes"),
        "{err}"
    );
}

#[test]
fn fill_errors_are_helpful() {
    let template = MiniMessage::new("<{c}>{victim}</{c}>").unwrap();
    let err = template.fill(&[("victim", "Notch".into())]).unwrap_err();
    assert!(
        err.to_string().contains("template needs a value for `{c}`"),
        "{err}"
    );

    let err = template
        .fill(&[("victim", "n".into()), ("c", "not a color".into())])
        .unwrap_err();
    assert!(err.to_string().contains("needs a Color value"), "{err}");
}

#[cfg(feature = "nbt")]
mod nbt_parity {
    use super::*;
    use simdnbt::owned::{BaseNbt, NbtCompound};
    use text_components::EncodedComponent;

    fn reference_bytes(c: &TextComponent) -> Vec<u8> {
        let mut compound = NbtCompound::new();
        compound.insert("v", c.to_codec_nbt());
        let mut out = Vec::new();
        BaseNbt::new("", compound).write(&mut out);
        out
    }

    fn wrap_mine(nbt: &[u8]) -> Vec<u8> {
        let mut out = vec![0x0A, 0, 0, nbt[0], 0, 1, b'v'];
        out.extend_from_slice(&nbt[1..]);
        out.push(0x00);
        out
    }

    fn parity(template: &str, values: &[(&str, Value)], macro_bytes: &EncodedComponent) {
        let parsed = MiniMessage::new(template)
            .unwrap_or_else(|err| panic!("{template:?} did not parse: {err}"));
        let runtime = parsed
            .fill_nbt(values)
            .unwrap_or_else(|err| panic!("{template:?} did not fill: {err}"));
        assert_eq!(
            &runtime, macro_bytes,
            "fill_nbt == text_nbt! for {template:?}"
        );
        let tree = parsed.fill(values).unwrap();
        assert_eq!(
            reference_bytes(&tree),
            wrap_mine(runtime.as_bytes()),
            "fill_nbt == fill + encoder for {template:?}"
        );
        runtime
            .decode()
            .unwrap_or_else(|err| panic!("{template:?} did not decode: {err}"));
    }

    macro_rules! parity {
        ($template:literal) => {
            parity($template, &[], &text_nbt!($template))
        };
        ($template:literal, $values:expr) => {
            parity($template, &$values, &text_nbt!($template))
        };
    }

    #[test]
    fn corpus_three_way_parity() {
        // {text: "", extra: []} is what the decoder (and vanilla) reject
        parity!("");
        parity!("<red></red>");
        parity!("<b>");
        parity!("<gradient:red:blue></gradient>");
        parity!("<hover:show_text:'hi'></hover>");
        parity!("<lang:multiplayer.player.left:''>");

        parity!("<gold><b>Steel</b></gold><gray> dev build</gray>");
        parity!("<gray>Press </gray><key:key.jump><gray> to fly</gray>");
        parity!("<lang_or:my.key:'Fallback text'>");
        parity!("<click:show_dialog:'steel:menu'>open</click>");
        parity!("<shadow:red:0.5>dim</shadow><!shadow>plain");
        parity!("<insert:'/msg Notch '>x</insert>");
        parity!(
            "<green><hover:show_text:'<yellow>Guild: Steel'>\
             <click:suggest_command:'/guild info Steel'>[STEEL] </click></hover></green>"
        );

        let name = "Notch".to_string();
        let c = Color::Aqua;
        let text_values = || vec![("name", Value::Text(name.clone()))];
        parity!("<red>{name}</red>", text_values());
        parity!("{name}", text_values());
        parity!(
            "<gradient:#000000:#FFFFFF>a{name}b</gradient>",
            text_values()
        );
        parity!(
            "<insertion:'/msg {name} '><click:run_command:'/msg {name}'>{name}</click></insertion>",
            text_values()
        );
        parity!(
            "<{c}>{name}</{c}>",
            [
                ("name", Value::Text(name.clone())),
                ("c", Value::Color(c.clone())),
            ]
        );

        // styled on purpose: a plain component encodes as an NBT string, changing the list form
        let styled = text!("<red><b>IN</b>NER</red>");
        let with_who = || vec![("who", Value::Component(styled.clone()))];
        {
            let who = styled.clone();
            parity!("{@who}", with_who());
        }
        {
            let who = styled.clone();
            parity!("x{@who}", with_who());
        }
        {
            let who = styled.clone();
            parity!("<gray>slain by </gray>{@who}", with_who());
        }
        {
            let who = styled.clone();
            parity!("<hover:show_text:'{@who}'>x</hover>", with_who());
        }
        {
            let who = styled.clone();
            parity!("<hover:show_text:'{@who}b'>x</hover>", with_who());
        }
        {
            let who = styled.clone();
            parity!(
                "<aqua><hover:show_text:'<red>{@who}'>x</hover></aqua>",
                with_who()
            );
        }
        {
            let who = styled.clone();
            parity!(
                "<lang:multiplayer.player.joined.renamed:'':'{@who}'>",
                with_who()
            );
        }
        {
            let name = "Notch".to_string();
            let arg = styled.clone();
            parity!(
                "<tr_or:my.key:'Hi {name}':'{@arg}'>",
                [
                    ("name", Value::Text("Notch".to_string())),
                    ("arg", Value::Component(styled.clone())),
                ]
            );
        }
    }

    #[test]
    fn lang_or_fallback_byte_parity() {
        const GREETING: EncodedComponent = text_nbt!("<lang_or:my.key:'Fallback text'>");

        let tree = text!("<lang_or:my.key:'Fallback text'>");
        assert_eq!(reference_bytes(&tree), wrap_mine(GREETING.as_bytes()));
        let runtime = MiniMessage::new("<lang_or:my.key:'Fallback text'>")
            .unwrap()
            .fill_nbt(&[])
            .unwrap();
        assert_eq!(GREETING, runtime);

        let who = text!("<aqua>Notch</aqua>");
        let name = "Notch".to_string();
        let arg = who.clone();
        let mine = text_nbt!("<tr_or:my.key:'Hi {name}':'{@arg}'>");
        let arg = who;
        let tree = text!("<tr_or:my.key:'Hi {name}':'{@arg}'>");
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn show_dialog_byte_parity() {
        let tree = text!("<click:show_dialog:'steel:menu'>open</click>");
        let mine = text_nbt!("<click:show_dialog:'steel:menu'>open</click>");
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));

        let template = MiniMessage::new("<click:show_dialog:'{id}'>open</click>").unwrap();
        let values: Vec<(&str, Value)> = vec![("id", "steel:menu".into())];
        let runtime_bytes = template.fill_nbt(&values).unwrap();
        assert_eq!(runtime_bytes, mine, "fill_nbt == text_nbt!");
        assert_eq!(
            reference_bytes(&template.fill(&values).unwrap()),
            wrap_mine(runtime_bytes.as_bytes())
        );
    }

    #[test]
    fn text_nbt_matches_tree_encoder() {
        let tree = text!(
            "<red>☠ </red><aqua>{}</aqua><gray> was slain by </gray><gold>{}</gold>",
            "Notch".to_string(),
            "Herobrine".to_string()
        );
        let mine = text_nbt!(
            "<red>☠ </red><aqua>{}</aqua><gray> was slain by </gray><gold>{}</gold>",
            "Notch",
            "Herobrine"
        );
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn static_text_nbt_is_rodata() {
        const MOTD: EncodedComponent = text_nbt!("<gold><b>Steel</b></gold>");
        let tree = text!("<gold><b>Steel</b></gold>");
        assert_eq!(reference_bytes(&tree), wrap_mine(MOTD.as_bytes()));
    }

    #[test]
    fn const_static_template_matches_fill_nbt() {
        const H: EncodedComponent = text_nbt!("<yellow>static</yellow>");
        let runtime = MiniMessage::new("<yellow>static</yellow>")
            .unwrap()
            .fill_nbt(&[])
            .unwrap();
        assert_eq!(H, runtime);
    }

    #[test]
    fn dynamic_color_and_flag_match_static_bytes() {
        let color = Color::Green;
        let dyn_bytes = text_nbt!("<{}>ok</{}>", &color);
        let static_bytes = text_nbt!("<green>ok</green>");
        assert_eq!(static_bytes, dyn_bytes);

        let dyn_flag = text_nbt!("<gold><obf:{}>x</obf></gold>", true);
        let static_flag = text_nbt!("<gold><obf>x</obf></gold>");
        assert_eq!(static_flag, dyn_flag);
    }

    #[test]
    fn color_tag_spellings_share_bytes() {
        assert_eq!(
            text_nbt!("<color:red>hot</color>"),
            text_nbt!("<red>hot</red>")
        );
        assert_eq!(
            text_nbt!("<color:#ff0000>hot</color>"),
            text_nbt!("<#ff0000>hot</#ff0000>")
        );
    }

    #[test]
    fn quoted_escape_byte_parity() {
        let template = MiniMessage::new("<hover:show_text:'don\\'t'>x</hover>").unwrap();
        let runtime_bytes = template.fill_nbt(&[]).unwrap();
        let macro_bytes = text_nbt!("<hover:show_text:'don\\'t'>x</hover>");
        assert_eq!(runtime_bytes, macro_bytes);
        assert_eq!(
            reference_bytes(&template.fill(&[]).unwrap()),
            wrap_mine(runtime_bytes.as_bytes())
        );
    }

    #[test]
    fn component_hole_matches_tree_encoder() {
        let fancy = text!("<aqua><hover:show_text:'Rank: MVP+'>Notch</hover></aqua>");
        let tree = text!("<red>☠ </red>{@}", fancy.clone());
        let mine = text_nbt!("<red>☠ </red>{@}", fancy.clone());
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn fill_nbt_three_way_parity() {
        let template = MiniMessage::new(
            "<red>\u{2620} </red><{c}>{victim}</{c}><gray> was slain by </gray>\
             {@killer}<obf:{magic}>!!</obf>",
        )
        .unwrap();
        let killer_component =
            text!("<gold><hover:show_text:'Rank: MVP+'>Herobrine</hover></gold>");
        let values: Vec<(&str, Value)> = vec![
            ("victim", "Notch".into()),
            ("killer", Value::Component(killer_component.clone())),
            ("c", Color::Aqua.into()),
            ("magic", true.into()),
        ];

        let runtime_bytes = template.fill_nbt(&values).unwrap();

        let c = Color::Aqua;
        let victim = "Notch";
        let killer = killer_component;
        let magic = true;
        let macro_bytes = text_nbt!(
            "<red>\u{2620} </red><{c}>{victim}</{c}><gray> was slain by </gray>\
             {@killer}<obf:{magic}>!!</obf>"
        );
        assert_eq!(runtime_bytes, macro_bytes, "fill_nbt == text_nbt!");

        let tree = template.fill(&values).unwrap();
        assert_eq!(
            reference_bytes(&tree),
            wrap_mine(runtime_bytes.as_bytes()),
            "fill_nbt == fill + encoder"
        );
    }

    #[test]
    fn fill_nbt_static_template() {
        let template = MiniMessage::new("<gold><b>Steel</b></gold>").unwrap();
        let bytes = template.fill_nbt(&[]).unwrap();
        let static_bytes = text_nbt!("<gold><b>Steel</b></gold>");
        assert_eq!(static_bytes, bytes);
    }

    #[test]
    fn hover_and_click_holes_byte_parity() {
        let template = MiniMessage::new(
            "<aqua><hover:show_text:'<yellow>Rank: {rank}'>\
             <click:suggest_command:'/msg {name} '>{name}</click></hover></aqua>",
        )
        .unwrap();
        let values: Vec<(&str, Value)> = vec![("rank", "MVP+".into()), ("name", "Notch".into())];

        let runtime_bytes = template.fill_nbt(&values).unwrap();

        let rank = "MVP+".to_string();
        let name = "Notch".to_string();
        let macro_bytes = text_nbt!(
            "<aqua><hover:show_text:'<yellow>Rank: {rank}'>\
             <click:suggest_command:'/msg {name} '>{name}</click></hover></aqua>"
        );
        assert_eq!(runtime_bytes, macro_bytes, "fill_nbt == text_nbt!");

        let tree = template.fill(&values).unwrap();
        assert_eq!(
            reference_bytes(&tree),
            wrap_mine(runtime_bytes.as_bytes()),
            "spliced bytes == tree + encoder"
        );
    }

    #[test]
    fn event_hole_byte_parity() {
        use text_components::interactivity::{ClickEvent, HoverEvent};

        let template =
            MiniMessage::new("<gold><hover:{item}><click:{cmd}>[Sword]</click></hover></gold>")
                .unwrap();
        let sword = HoverEvent::show_item("minecraft:diamond_sword", Some(3), None);
        let give = ClickEvent::run_command("/give @s diamond_sword");
        let values: Vec<(&str, Value)> =
            vec![("item", sword.clone().into()), ("cmd", give.clone().into())];

        let runtime_bytes = template.fill_nbt(&values).unwrap();

        let item = sword;
        let cmd = give;
        let macro_bytes =
            text_nbt!("<gold><hover:{item}><click:{cmd}>[Sword]</click></hover></gold>");
        assert_eq!(runtime_bytes, macro_bytes, "fill_nbt == text_nbt!");

        let tree = template.fill(&values).unwrap();
        assert_eq!(
            reference_bytes(&tree),
            wrap_mine(runtime_bytes.as_bytes()),
            "event bytes == tree + encoder"
        );
    }

    #[test]
    fn keybind_and_style_fields_byte_parity() {
        let tree = text!(
            "<gray>Press </gray><key:key.jump><font:minecraft:uniform>\
             <shadow:#80FF0000><insertion:'jump'> to fly</insertion></shadow></font>"
        );
        let mine = text_nbt!(
            "<gray>Press </gray><key:key.jump><font:minecraft:uniform>\
             <shadow:#80FF0000><insertion:'jump'> to fly</insertion></shadow></font>"
        );
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));

        let template = MiniMessage::new(
            "<gray>Press </gray><key:key.jump><insertion:'/msg {name} '> to fly</insertion>",
        )
        .unwrap();
        let values: Vec<(&str, Value)> = vec![("name", "Notch".into())];
        let runtime_bytes = template.fill_nbt(&values).unwrap();
        assert_eq!(
            reference_bytes(&template.fill(&values).unwrap()),
            wrap_mine(runtime_bytes.as_bytes())
        );
    }

    #[test]
    fn gradient_byte_parity() {
        let tree = text!("<gradient:red:blue>Steel</gradient>");
        let mine = text_nbt!("<gradient:red:blue>Steel</gradient>");
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn unicode_mutf8_parity() {
        let tree = text!("<red>{}</red>", "Herobrine ☠ 💀".to_string());
        let mine = text_nbt!("<red>{}</red>", "Herobrine ☠ 💀");
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn format_spec_matches_preformatted_string() {
        let mspt = 12.3456_f64;
        let spec = text_nbt!("<gray>MSPT: </gray><green>{:.2}</green>", mspt);
        let pre = text_nbt!("<gray>MSPT: </gray><green>{}</green>", format!("{mspt:.2}"));
        assert_eq!(spec, pre);

        let tree = text!("<gray>MSPT: </gray><green>{:.2}</green>", mspt);
        assert_eq!(reference_bytes(&tree), wrap_mine(spec.as_bytes()));
    }

    #[test]
    fn named_and_debug_and_hex_specs() {
        let name = "Notch";
        let debug = text_nbt!("<red>{name:?}</red>");
        assert_eq!(debug, text_nbt!("<red>{}</red>", format!("{name:?}")));

        let n = 255u32;
        let hex = text_nbt!("<gold>{:#x}</gold>", n);
        assert_eq!(hex, text_nbt!("<gold>{}</gold>", format!("{n:#x}")));

        let tree = text!("<red>{name:?}</red>");
        assert_eq!(reference_bytes(&tree), wrap_mine(debug.as_bytes()));
    }

    #[test]
    fn spec_inside_click_value() {
        let coord = 7.5_f64;
        let mine = text_nbt!(
            "<click:run_command:'/tp {:.1} 64 {:.1}'>go</click>",
            coord,
            coord
        );
        let pre = text_nbt!(
            "<click:run_command:'/tp {} 64 {}'>go</click>",
            format!("{coord:.1}"),
            format!("{coord:.1}")
        );
        assert_eq!(mine, pre);

        let tree = text!(
            "<click:run_command:'/tp {:.1} 64 {:.1}'>go</click>",
            coord,
            coord
        );
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn lang_tag_byte_parity() {
        use text_components::translation::Translation;

        static LEFT: Translation<1> = Translation("multiplayer.player.left");

        let tree = text!("<yellow><lang:multiplayer.player.left></yellow>");
        let mine = text_nbt!("<yellow><lang:multiplayer.player.left></yellow>");
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));

        let tree = text!("<lang:multiplayer.player.left:'Notch'>");
        let mine = text_nbt!("<lang:multiplayer.player.left:'Notch'>");
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));

        let display = text!("<aqua><b>Notch</b></aqua>");
        let tree = text!("<yellow><lang:{}:'{@}'></yellow>", &LEFT, display.clone());
        let mine = text_nbt!("<yellow><lang:{}:'{@}'></yellow>", &LEFT, display.clone());
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn const_lang_args_match_runtime_bytes() {
        const TEMPLATE: &str = "<yellow><lang:multiplayer.player.joined.renamed:\
                                '<aqua><b>Notch</b></aqua>':'Herobrine'></yellow>";
        const CONST_TREE: TextComponent = text!(
            "<yellow><lang:multiplayer.player.joined.renamed:\
             '<aqua><b>Notch</b></aqua>':'Herobrine'></yellow>"
        );

        let runtime = MiniMessage::new(TEMPLATE).unwrap();
        assert_eq!(
            reference_bytes(&CONST_TREE),
            reference_bytes(&runtime.fill(&[]).unwrap()),
            "const Args::Static == runtime Args::Owned"
        );
        assert_eq!(
            reference_bytes(&CONST_TREE),
            wrap_mine(runtime.fill_nbt(&[]).unwrap().as_bytes())
        );

        let mine = text_nbt!(
            "<yellow><lang:multiplayer.player.joined.renamed:\
             '<aqua><b>Notch</b></aqua>':'Herobrine'></yellow>"
        );
        assert_eq!(reference_bytes(&CONST_TREE), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn lang_key_arity_is_typed() {
        use text_components::translation::Translation;

        // the tag's argument count picks the Translation<N> the key is borrowed as
        static JOINED: Translation<2> = Translation("multiplayer.player.joined.renamed");
        let new_name = text!("<aqua>Notch</aqua>");
        let mine = text_nbt!(
            "<yellow><lang:{}:'{@}':'{}'></yellow>",
            &JOINED,
            new_name.clone(),
            "Herobrine"
        );
        let tree = text!(
            "<yellow><lang:{}:'{@}':'{}'></yellow>",
            &JOINED,
            new_name.clone(),
            "Herobrine".to_string()
        );
        assert_eq!(reference_bytes(&tree), wrap_mine(mine.as_bytes()));
    }

    #[test]
    fn lang_three_way_parity() {
        use text_components::translation::Translation;

        static KEY: Translation<2> = Translation("multiplayer.player.joined.renamed");

        let template = MiniMessage::new("<yellow><lang:{key}:'{@who}':'{old}'></yellow>").unwrap();
        let who = text!("<aqua>Notch</aqua>");
        let values: Vec<(&str, Value)> = vec![
            ("key", "multiplayer.player.joined.renamed".into()),
            ("who", Value::Component(who.clone())),
            ("old", "Herobrine".into()),
        ];
        let runtime_bytes = template.fill_nbt(&values).unwrap();

        let tree = template.fill(&values).unwrap();
        assert_eq!(
            reference_bytes(&tree),
            wrap_mine(runtime_bytes.as_bytes()),
            "fill_nbt == fill + encoder"
        );

        let macro_bytes = text_nbt!(
            "<yellow><lang:{}:'{@}':'{}'></yellow>",
            &KEY,
            who.clone(),
            "Herobrine"
        );
        assert_eq!(runtime_bytes, macro_bytes, "fill_nbt == text_nbt!");
    }

    #[test]
    fn translation_encode_matches_codec() {
        use text_components::translation::{TranslatedMessage, Translation};

        static LEFT: Translation<1> = Translation("multiplayer.player.left");
        static SPAM: Translation<0> = Translation("disconnect.spam");

        let message = LEFT.message([text!("<aqua>Notch</aqua>")]);
        assert_eq!(
            reference_bytes(&TextComponent::translated(message.clone())),
            wrap_mine(message.encode().as_bytes()),
            "TranslatedMessage::encode == tree + encoder"
        );

        let bare = SPAM.msg();
        assert_eq!(
            reference_bytes(&TextComponent::translated(bare.clone())),
            wrap_mine(bare.encode().as_bytes())
        );

        // the codec distinguishes `message([])`'s empty list from `msg()`'s missing field
        let empty: [TextComponent; 0] = [];
        let empty = SPAM.message(empty);
        assert_eq!(
            reference_bytes(&TextComponent::translated(empty.clone())),
            wrap_mine(empty.encode().as_bytes())
        );
        assert_ne!(empty.encode().as_bytes(), bare.encode().as_bytes());

        let plain = LEFT.message([TextComponent::plain("Notch")]);
        assert_eq!(
            reference_bytes(&TextComponent::translated(plain.clone())),
            wrap_mine(plain.encode().as_bytes())
        );

        let with_fallback = TranslatedMessage {
            fallback: Some("%s left the game".into()),
            ..LEFT.message([TextComponent::plain("Notch")])
        };
        assert_eq!(
            reference_bytes(&TextComponent::translated(with_fallback.clone())),
            wrap_mine(with_fallback.encode().as_bytes())
        );
    }
}

#[cfg(feature = "nbt")]
mod decoding {
    use super::*;
    use text_components::EncodedComponent;
    use text_components::content::Resolvable;
    #[cfg(feature = "custom")]
    use text_components::custom::CustomData;
    use text_components::resolving::TextResolutor;

    struct Language;

    impl TextResolutor for Language {
        fn resolve_content(&self, _resolvable: &Resolvable) -> TextComponent {
            TextComponent::new()
        }

        #[cfg(feature = "custom")]
        fn resolve_custom(&self, _data: &CustomData) -> Option<TextComponent> {
            None
        }

        fn translate(&self, key: &str) -> Option<String> {
            match key {
                "multiplayer.player.joined.renamed" => {
                    Some("%s joined the game (formerly known as %s)".to_string())
                }
                _ => None,
            }
        }
    }

    #[test]
    fn decode_round_trips_to_the_parsed_tree() {
        let template = "<red>\u{2620} </red><aqua><b>Notch</b></aqua>\
                        <gray> was slain by </gray><gold>Herobrine</gold>";
        let decoded = text_nbt!(
            "<red>\u{2620} </red><aqua><b>Notch</b></aqua>\
             <gray> was slain by </gray><gold>Herobrine</gold>"
        )
        .decode()
        .expect("text_nbt! output should decode");

        assert_eq!(decoded, parse(template));
        assert_eq!(
            decoded,
            text!(
                "<red>\u{2620} </red><aqua><b>Notch</b></aqua>\
                 <gray> was slain by </gray><gold>Herobrine</gold>"
            )
        );
    }

    #[test]
    fn decode_round_trips_a_template_with_holes() {
        let who = text!("<aqua><b>Notch</b></aqua>");
        let encoded = text_nbt!(
            "<yellow><lang:multiplayer.player.joined.renamed:'{@}':'{}'></yellow>",
            who.clone(),
            "Herobrine"
        );

        assert_eq!(
            encoded.decode().expect("holes should decode"),
            text!(
                "<yellow><lang:multiplayer.player.joined.renamed:'{@}':'{}'></yellow>",
                who,
                "Herobrine"
            )
        );
    }

    #[test]
    fn to_plain_resolves_translation_arguments() {
        let encoded = text_nbt!(
            "<yellow><lang:multiplayer.player.joined.renamed:'{@}':'{}'></yellow>",
            text!("<aqua>Notch</aqua>"),
            "Herobrine"
        );

        assert_eq!(
            encoded.to_plain(&Language).expect("should decode"),
            "Notch joined the game (formerly known as Herobrine)"
        );
    }

    #[test]
    fn component_hole_is_a_whole_hover_value() {
        use text_components::interactivity::HoverEvent;

        let inner = text!("<red><b>IN</b>NER</red>");
        let a = inner.clone();
        let encoded = text_nbt!("<hover:show_text:'{@a}'>x</hover>");
        let decoded = encoded.decode().expect("a spliced hover should decode");
        match decoded.interactions.hover.as_deref() {
            Some(HoverEvent::ShowText { value }) => assert_eq!(**value, inner),
            other => panic!("expected show_text, got {other:?}"),
        }
    }

    #[test]
    fn malformed_bytes_are_an_error_not_a_panic() {
        // tag type 8 (string) with a length promising more bytes than follow
        let truncated = EncodedComponent::from_static(&[8, 0, 9, b'h', b'i']);
        let error = truncated
            .decode()
            .expect_err("a truncated string should not decode");
        assert!(error.to_string().contains("malformed NBT"), "{error}");

        // a well-formed tag that is not a component
        let not_a_component = EncodedComponent::from_static(&[3, 0, 0, 0, 7]);
        let error = not_a_component
            .decode()
            .expect_err("an int is not a component");
        assert_eq!(error.to_string(), "expected a text component");
    }
}

#[cfg(feature = "nbt")]
mod splicing {
    use super::*;
    use text_components::EncodedComponent;
    use text_components::format::Color;

    const PREFIX: EncodedComponent = text_nbt!("<gold>[Steel] </gold>");
    const BARE: EncodedComponent = text_nbt!("plain");

    fn encoded_parity(template: &str, values: &[(&str, Value)], macro_bytes: &EncodedComponent) {
        let runtime = MiniMessage::new(template)
            .unwrap_or_else(|err| panic!("{template:?} did not parse: {err}"))
            .fill_nbt(values)
            .unwrap_or_else(|err| panic!("{template:?} did not fill: {err}"));
        assert_eq!(
            &runtime, macro_bytes,
            "fill_nbt == text_nbt! for {template:?}"
        );
        runtime
            .decode()
            .unwrap_or_else(|err| panic!("{template:?} did not decode: {err}"));
    }

    #[test]
    fn encoded_holes_splice_by_position() {
        {
            let x = text_nbt!("<red>hi</red>");
            assert_eq!(text_nbt!("{@x}"), text_nbt!("<red>hi</red>"));
        }
        {
            let x = text_nbt!("<red>hi</red>");
            assert_eq!(text_nbt!("a{@x}"), text_nbt!("a<red>hi</red>"));
        }
        {
            let x = text_nbt!("<red>hi</red>");
            assert_eq!(
                text_nbt!("<hover:show_text:'{@x}'>y</hover>"),
                text_nbt!("<hover:show_text:'<red>hi</red>'>y</hover>")
            );
        }
        {
            let x = text_nbt!("<red>hi</red>");
            assert_eq!(
                text_nbt!("<lang:multiplayer.player.left:'{@x}'>"),
                text_nbt!("<lang:multiplayer.player.left:'<red>hi</red>'>")
            );
        }
        // a bare (string-encoded) component takes the {"": …} list-element wrapper
        {
            let x = BARE;
            assert_eq!(
                text_nbt!("a{@x}").decode().unwrap(),
                text!("a{@}", TextComponent::plain("plain"))
            );
        }
        {
            let x = BARE;
            assert_eq!(text_nbt!("{@x}"), BARE);
        }
    }

    #[test]
    fn styled_encoded_holes_wrap_instead_of_restyling() {
        let x = text_nbt!("<red>hi</red>");
        let decoded = text_nbt!("<gray><b>{@x}</b></gray>").decode().unwrap();
        assert_eq!(decoded.format.color, Some(Color::Gray));
        assert_eq!(decoded.format.bold, Some(true));
        assert_eq!(decoded.children.len(), 1);
        assert_eq!(decoded.children[0].format.color, Some(Color::Red));

        let x = BARE;
        let decoded = text_nbt!("<gray>{@x}</gray>").decode().unwrap();
        assert_eq!(decoded.format.color, Some(Color::Gray));
        assert!(decoded.children[0].format.color.is_none());

        let x = PREFIX;
        assert_eq!(
            text_nbt!("<gray>{@x}</gray>"),
            text_nbt!("<gray>{@const PREFIX}</gray>")
        );
    }

    #[test]
    fn fill_nbt_splices_encoded_values() {
        let value = |template: &str| -> Vec<(&str, Value)> {
            let _ = template;
            vec![("x", Value::Encoded(text_nbt!("<red>hi</red>")))]
        };
        {
            let x = text_nbt!("<red>hi</red>");
            encoded_parity("{@x}", &value("{@x}"), &text_nbt!("{@x}"));
        }
        {
            let x = text_nbt!("<red>hi</red>");
            encoded_parity("a{@x}", &value("a{@x}"), &text_nbt!("a{@x}"));
        }
        {
            let x = text_nbt!("<red>hi</red>");
            encoded_parity(
                "<hover:show_text:'{@x}'>y</hover>",
                &value(""),
                &text_nbt!("<hover:show_text:'{@x}'>y</hover>"),
            );
        }
        {
            let x = text_nbt!("<red>hi</red>");
            encoded_parity("<gray><b>{@x}</b></gray>", &value(""), &{
                text_nbt!("<gray><b>{@x}</b></gray>")
            });
        }
        let template = MiniMessage::new("<gray>{@x}</gray>").unwrap();
        assert!(
            template
                .fill_nbt(&[("x", Value::Component(text!("<red>hi</red>")))])
                .is_ok()
        );
        let err = template
            .fill(&[("x", Value::Encoded(BARE))])
            .expect_err("fill() cannot put bytes in a tree");
        assert!(err.to_string().contains("EncodedComponent"), "{err}");
    }

    #[test]
    fn const_splices_fold_into_rodata() {
        const MSG: EncodedComponent = text_nbt!("{@const PREFIX}<gray>hello</gray>");
        const ROOT: EncodedComponent = text_nbt!("{@const PREFIX}");
        const HOVER: EncodedComponent = text_nbt!("<hover:show_text:'{@const PREFIX}'>x</hover>");
        const LANG: EncodedComponent =
            text_nbt!("<lang:multiplayer.player.left:'{@const PREFIX}'>");
        const WRAPPED: EncodedComponent = text_nbt!("a{@const BARE}");

        assert_eq!(MSG, text_nbt!("<gold>[Steel] </gold><gray>hello</gray>"));
        assert_eq!(ROOT, PREFIX);
        assert_eq!(
            HOVER,
            text_nbt!("<hover:show_text:'<gold>[Steel] </gold>'>x</hover>")
        );
        assert_eq!(
            LANG,
            text_nbt!("<lang:multiplayer.player.left:'<gold>[Steel] </gold>'>")
        );
        assert_eq!(
            WRAPPED.decode().unwrap(),
            text!("a{@}", TextComponent::plain("plain"))
        );

        let decoded = text_nbt!("<gray>{@const PREFIX}</gray>").decode().unwrap();
        assert_eq!(decoded.format.color, Some(Color::Gray));
        assert_eq!(decoded.children[0].format.color, Some(Color::Gold));
    }

    #[test]
    fn const_and_runtime_holes_mix() {
        let name = "Notch".to_string();
        assert_eq!(
            text_nbt!("{@const PREFIX}<gray>{name}</gray>"),
            text_nbt!("<gold>[Steel] </gold><gray>{name}</gray>")
        );

        let who = text_nbt!("<aqua>Notch</aqua>");
        assert_eq!(
            text_nbt!("{@const PREFIX}{@who}<gray> joined</gray>"),
            text_nbt!("<gold>[Steel] </gold><aqua>Notch</aqua><gray> joined</gray>")
        );
    }

    #[test]
    fn screaming_holes_hear_about_const_splicing() {
        const HEADER: TextComponent = text!("<gold>hi</gold>");
        let _ = text_nbt!("{@HEADER}!");
        #[expect(
            deprecated,
            reason = "the const-splicing hint is the point of this test"
        )]
        let spliced = text_nbt!("{@PREFIX}!");
        assert_eq!(spliced, text_nbt!("{@const PREFIX}!"));
    }

    #[test]
    fn const_splices_are_rejected_at_runtime() {
        let err = MiniMessage::new("<red>{@const HEADER}</red>").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("only works in the text_nbt! macro"), "{msg}");
        assert!(msg.contains("pass the component as a value"), "{msg}");
    }
}
