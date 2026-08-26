//! Malformed input errors instead of panicking, and equivalent spellings parse identically.
#![expect(clippy::unwrap_used, reason = "tests unwrap known-good input")]

use text_components_grammar::nbt::{NbtSeg, Unresolved, emit, to_mutf8};
use text_components_grammar::{
    ColorIr, HeadIr, HoleArg, HoleKind, HoverIr, Mode, NbtSourceIr, Piece, StrSeg, Style, parse,
};

fn pieces(input: &str, mode: Mode) -> Vec<Piece> {
    match parse(input, mode) {
        Ok(template) => template.pieces,
        Err(err) => panic!("{}", err.render(input)),
    }
}

fn text_of(pieces: &[Piece]) -> String {
    pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn dyn_color_name(input: &str) -> String {
    match pieces(input, Mode::Runtime)[0].style().color.clone() {
        Some(ColorIr::Dyn(HoleArg::Named { name, .. })) => name,
        other => panic!("{input:?}: expected a named dynamic color, got {other:?}"),
    }
}

#[test]
fn malformed_input_errors() {
    for input in [
        "<color>",
        "<color:red:blue>",
        "<color:mauve>",
        "<shadow:#€abcde>",
        "<shadow:#zzzzzzzz>",
        "<red",
        "<hover:show_text:'oops>x",
        "<insertion:'a'b'c'>",
        "<rainbow:red>",
        "<gradient:#ff0000>x</gradient>",
        "</gold>",
        "hello {}",
        "{name",
        // a color tag has nothing to do with an argument
        "<red:blue>x",
        // hole names become variables in the macros: `²` and a lone `_` are not identifiers
        "{a²}",
        "<{a²}>x",
        "{_}",
        "<score:a>",
        "<score:a:b:c>",
        "<score:{x}:b>",
        "<selector>",
        "<nbt:chest:'0 0 0':Items>",
        "<nbt:entity:'@s':Health:' ':nope>",
        "<sprite>",
        "<sprite:a:b:c>",
        "<head:'a name that is far too long'>",
        "<head:Notch:maybe>",
        "<transition:red:blue:2>",
        "<transition:0.5>",
        "<hover:show_entity:pig:not-a-uuid>",
        "<hover:show_item:'minecraft:stone':many>",
        "<hover:show_item:'minecraft:stone':1:'{}'>",
        "<hover:show_entity:pig:1f085b2d-9548-4159-a8c7-f3ccdf0c2054:''>",
    ] {
        let Err(err) = parse(input, Mode::Runtime) else {
            panic!("{input:?} parsed but should not");
        };
        assert!(
            err.render(input).contains(&err.message),
            "{input:?} rendered without its message"
        );
    }
}

/// Text and styling, dropping the source ranges.
fn shape(input: &str, mode: Mode) -> (String, Vec<Style>) {
    let pieces = pieces(input, mode);
    (
        text_of(&pieces),
        pieces.iter().map(|p| p.style().clone()).collect(),
    )
}

#[test]
fn color_tag_matches_the_shorthands() {
    for (long, short) in [
        ("<color:red>x</color>", "<red>x</red>"),
        ("<color:#ff0000>x</color>", "<#ff0000>x</#ff0000>"),
        ("<color:{}>x</color>", "<{}>x</{}>"),
    ] {
        assert_eq!(
            shape(long, Mode::Macro),
            shape(short, Mode::Macro),
            "{long:?} vs {short:?}"
        );
    }
    // a named hole records where it was written, so compare the name instead
    assert_eq!(dyn_color_name("<color:{c}>x</color>"), "c");
    assert_eq!(dyn_color_name("<{c}>x</{c}>"), "c");
}

#[test]
fn close_color_ends_a_dynamic_color() {
    let pieces = pieces("<{c}>a</color>b", Mode::Runtime);
    assert!(matches!(pieces[0].style().color, Some(ColorIr::Dyn(_))));
    assert!(pieces[1].style().color.is_none());
}

#[test]
fn quotes_unescape() {
    let hover = pieces("<hover:show_text:'don\\'t'>x</hover>", Mode::Runtime)[0]
        .style()
        .hover
        .clone();
    let Some(HoverIr::Text(text)) = hover else {
        panic!("expected hover text, got {hover:?}");
    };
    assert_eq!(text_of(&text), "don't");

    let insertion = pieces("<insertion:'a\\\\b'>x", Mode::Runtime)[0]
        .style()
        .insertion
        .clone();
    assert_eq!(
        insertion.as_deref().and_then(StrSeg::join_lits).as_deref(),
        Some("a\\b")
    );
}

#[test]
fn markup_escapes_stay_literal() {
    assert_eq!(
        text_of(&pieces(r"\<literal {{}} \\", Mode::Runtime)),
        r"<literal {} \"
    );
}

#[test]
fn const_holes_name_their_const() {
    let styled = pieces("<red>{@const HEADER}</red>", Mode::Macro);
    let Piece::Hole { arg, kind, .. } = &styled[0] else {
        panic!("expected a hole, got {:?}", styled[0]);
    };
    assert_eq!(*kind, HoleKind::ConstComponent);
    assert!(matches!(arg, HoleArg::Named { name, .. } if name == "HEADER"));

    for input in ["{@const}", "{@const }", "{@const a b}"] {
        assert!(
            parse(input, Mode::Macro).is_err(),
            "{input:?} parsed but should not"
        );
    }
    assert!(matches!(
        &pieces("{@constant}", Mode::Macro)[0],
        Piece::Hole {
            kind: HoleKind::Component,
            ..
        }
    ));
    assert!(parse("{@const HEADER}", Mode::Runtime).is_err());
}

#[test]
fn const_text_holes_name_their_const() {
    let styled = pieces("<red>{const GIT_HASH}</red>", Mode::Macro);
    let Piece::Hole { arg, kind, .. } = &styled[0] else {
        panic!("expected a hole, got {:?}", styled[0]);
    };
    assert_eq!(*kind, HoleKind::ConstText);
    assert!(matches!(arg, HoleArg::Named { name, .. } if name == "GIT_HASH"));
    assert!(
        !styled[0].has_dyn(),
        "a const text hole is not runtime data"
    );

    for input in ["{const}", "{const }", "{const A:.2}", "{const a b}"] {
        assert!(
            parse(input, Mode::Macro).is_err(),
            "{input:?} parsed but should not"
        );
    }
    assert!(matches!(
        &pieces("{constant}", Mode::Macro)[0],
        Piece::Hole {
            kind: HoleKind::Text,
            ..
        }
    ));
    assert!(parse("{const GIT_HASH}", Mode::Runtime).is_err());
}

#[test]
fn const_text_holes_stay_in_text_position() {
    for input in [
        "<insert:'{const A}'>x</insert>",
        "<click:run_command:'{const A}'>x</click>",
        "<lang_or:my.key:'{const A}'>",
        "<{const A}>x",
        "<b:{const A}>x</b>",
    ] {
        let Err(err) = parse(input, Mode::Macro) else {
            panic!("{input:?} parsed but should not");
        };
        assert!(
            err.render(input).contains("text position"),
            "{input:?}: {}",
            err.message
        );
    }
}

#[test]
fn const_text_holes_take_one_gradient_position() {
    let over_const = pieces("<gradient:red:blue>{const A}</gradient>", Mode::Macro);
    let over_hole = pieces("<gradient:red:blue>{}</gradient>", Mode::Macro);
    assert_eq!(over_const.len(), 1);
    assert_eq!(over_const[0].style().color, over_hole[0].style().color);
}

#[test]
fn const_text_holes_fill_lang_args() {
    let lang = pieces("<lang:my.key:'{const A}'>", Mode::Macro);
    let Piece::Lang { args, .. } = &lang[0] else {
        panic!("expected a lang piece, got {:?}", lang[0]);
    };
    assert!(matches!(
        &args[0][0],
        Piece::Hole {
            kind: HoleKind::ConstText,
            ..
        }
    ));
    assert!(!lang[0].has_dyn(), "a const lang arg is not runtime data");
}

#[test]
fn hole_names_are_identifiers() {
    for input in ["{name}", "{_name}", "{n2}", "{café}"] {
        assert!(
            parse(input, Mode::Runtime).is_ok(),
            "{input:?} should parse"
        );
    }
}

#[test]
fn empty_template_has_no_pieces() {
    assert_eq!(
        parse("", Mode::Runtime).unwrap().pieces,
        [] as [text_components_grammar::Piece; 0]
    );
}

#[test]
fn text_producing_tags() {
    assert_eq!(text_of(&pieces("a<newline>b", Mode::Runtime)), "a\nb");
    let reset = pieces("<red>a<reset>b", Mode::Runtime);
    assert!(reset[0].style().color.is_some());
    assert!(reset[1].style().color.is_none());
}

#[test]
fn emit_errors_point_at_the_piece_that_failed() {
    // one character over what an NBT string can carry
    let long = "a".repeat(65536);
    let template = format!("<red>ok</red><gold>{long}</gold>");
    let Err(err) = emit(&pieces(&template, Mode::Runtime)) else {
        panic!("a 65536-byte string should not encode");
    };
    assert_eq!(err.range, Some((19, 19 + long.chars().count())));
}

#[test]
fn open_tags_close_at_the_end() {
    assert_eq!(
        pieces("<red>x", Mode::Runtime),
        pieces("<red>x</red>", Mode::Runtime)
    );
}

fn shadow_of(input: &str) -> Option<i32> {
    pieces(input, Mode::Runtime)[0].style().shadow_color
}

#[test]
fn shadow_alpha_defaults_to_a_quarter() {
    assert_eq!(shadow_of("<shadow:red>x"), Some(0x40FF_5555_u32 as i32));
    assert_eq!(shadow_of("<shadow:#ff0000>x"), Some(0x40FF_0000_u32 as i32));
    assert_eq!(
        shadow_of("<shadow:#80FF0000>x"),
        Some(0x80FF_0000_u32 as i32)
    );
    assert_eq!(shadow_of("<shadow:red:0.5>x"), Some(0x80FF_5555_u32 as i32));
    assert_eq!(
        shadow_of("<shadow:#ff0000:0.5>x"),
        Some(0x80FF_0000_u32 as i32)
    );
    assert_eq!(shadow_of("<shadow:red:0>x"), Some(0x00FF_5555));
    assert_eq!(shadow_of("<shadow:red:1>x"), Some(0xFFFF_5555_u32 as i32));
    assert_eq!(shadow_of("<!shadow>x"), Some(0));

    for input in [
        "<shadow:red:2>x",
        "<shadow:red:-0.5>x",
        "<shadow:red:half>x",
        "<shadow:red:0.5:0.5>x",
        "<!shadow:red>x",
        "<!red>x",
    ] {
        assert!(
            parse(input, Mode::Runtime).is_err(),
            "{input:?} parsed but should not"
        );
    }
}

#[test]
fn tag_names_are_case_insensitive() {
    for (upper, lower) in [
        ("<RED>x</Red>", "<red>x</red>"),
        ("<C:blue>x</c>", "<color:blue>x</color>"),
        ("<COLOUR:#00FF00>x", "<color:#00ff00>x"),
        ("<Shadow:red>x", "<shadow:red>x"),
        ("<B>x</B>", "<bold>x</bold>"),
        ("a<NewLine>b", "a<newline>b"),
        ("<Key:key.jump>", "<key:key.jump>"),
    ] {
        assert_eq!(
            shape(upper, Mode::Runtime),
            shape(lower, Mode::Runtime),
            "{upper:?} vs {lower:?}"
        );
    }
    // a hole names a variable, so its case survives
    assert_eq!(dyn_color_name("<{Color}>x</{Color}>"), "Color");
    assert_eq!(
        shape("<Font:Minecraft:Uniform>x", Mode::Runtime).1[0].font,
        Some("Minecraft:Uniform".to_string())
    );
}

#[test]
fn double_quotes_work_like_single_quotes() {
    let hover = |input: &str| match pieces(input, Mode::Runtime)[0].style().hover.clone() {
        Some(HoverIr::Text(text)) => text_of(&text),
        other => panic!("{input:?}: expected hover text, got {other:?}"),
    };
    assert_eq!(hover("<hover:show_text:\"hi\">x</hover>"), "hi");
    assert_eq!(hover("<hover:show_text:\"it's\">x</hover>"), "it's");
    assert_eq!(
        hover("<hover:show_text:\"say \\\"hi\\\"\">x</hover>"),
        "say \"hi\""
    );
    assert_eq!(
        hover("<hover:show_text:'say \"hi\"'>x</hover>"),
        "say \"hi\""
    );

    let err = parse("<hover:show_text:\"a'>x", Mode::Runtime).unwrap_err();
    assert!(err.message.contains("unclosed `\"`"), "{}", err.message);
    let err = parse("<insertion:\"a\"b\">x", Mode::Runtime).unwrap_err();
    assert!(
        err.message.contains("must span the whole argument"),
        "{}",
        err.message
    );
}

#[test]
fn self_closing_tags_close_themselves() {
    // the source ranges differ, so compare text and styling
    assert_eq!(
        shape("<key:key.jump/>x", Mode::Runtime),
        shape("<key:key.jump>x", Mode::Runtime)
    );
    assert!(matches!(
        &pieces("<key:key.jump/>x", Mode::Runtime)[0],
        Piece::Keybind { key, .. } if key == "key.jump"
    ));
    assert_eq!(shape("<red/>x", Mode::Runtime), shape("x", Mode::Runtime));
    assert_eq!(
        shape("<red/>x<red>y", Mode::Runtime),
        shape("x<red>y</red>", Mode::Runtime)
    );
    assert_eq!(text_of(&pieces("a<newline/>b", Mode::Runtime)), "a\nb");
    assert_eq!(
        pieces("<font:a/b>x", Mode::Runtime)[0].style().font,
        Some("a/b".to_string())
    );
    assert_eq!(
        shape("<insert:'x'/>y", Mode::Runtime),
        shape("y", Mode::Runtime)
    );
}

#[test]
fn lang_or_carries_a_fallback() {
    let fallback = |input: &str| match &pieces(input, Mode::Runtime)[0] {
        Piece::Lang { fallback, args, .. } => {
            (fallback.as_deref().and_then(StrSeg::join_lits), args.len())
        }
        other => panic!("{input:?}: expected a translation, got {other:?}"),
    };
    assert_eq!(
        fallback("<lang_or:my.key:'Fallback text'>"),
        (Some("Fallback text".to_string()), 0)
    );
    assert_eq!(
        fallback("<tr_or:my.key:'Fallback':'a':'b'>"),
        (Some("Fallback".to_string()), 2)
    );
    assert_eq!(
        fallback("<translate_or:my.key:'Fallback'>"),
        (Some("Fallback".to_string()), 0)
    );
    assert_eq!(fallback("<lang:my.key:'a'>"), (None, 1));

    let err = parse("<lang_or:my.key>", Mode::Runtime).unwrap_err();
    assert!(err.message.contains("needs a fallback"), "{}", err.message);
}

const NOTCH: [u8; 16] = [
    0x1f, 0x08, 0x5b, 0x2d, 0x95, 0x48, 0x41, 0x59, 0xa8, 0xc7, 0xf3, 0xcc, 0xdf, 0x0c, 0x20, 0x54,
];

#[test]
fn content_tag_aliases_parse_the_same() {
    let selector = |input: &str| match &pieces(input, Mode::Runtime)[0] {
        Piece::Selector { selector, .. } => selector.clone(),
        other => panic!("{input:?}: expected a selector, got {other:?}"),
    };
    assert_eq!(selector("<sel:@a>"), selector("<selector:@a>"));

    let source = |input: &str| match &pieces(input, Mode::Runtime)[0] {
        Piece::Nbt { source, path, .. } => (source.clone(), path.clone()),
        other => panic!("{input:?}: expected an nbt piece, got {other:?}"),
    };
    assert_eq!(
        source("<data:entity:'@s':Health>"),
        source("<nbt:entity:'@s':Health>")
    );
    assert_eq!(
        source("<nbt:storage:'my:key':path>").0,
        NbtSourceIr::Storage("my:key".to_string())
    );
}

#[test]
fn transition_picks_one_color_off_the_gradient() {
    let color = |input: &str| pieces(input, Mode::Runtime)[0].style().color.clone();
    assert_eq!(
        color("<transition:red:blue:1>x"),
        Some(ColorIr::Rgb(0x55, 0x55, 0xFF))
    );
    assert_eq!(
        color("<transition:red:blue:-0.5>x"),
        color("<transition:red:blue:0.5>x")
    );
    assert_eq!(
        color("<transition:red:blue>x"),
        color("<transition:red:blue:0>x")
    );
}

#[test]
fn uuids_take_both_spellings() {
    let hover = |input: &str| pieces(input, Mode::Runtime)[0].style().hover.clone();
    assert_eq!(
        hover("<hover:show_entity:pig:1f085b2d-9548-4159-a8c7-f3ccdf0c2054>x"),
        hover("<hover:show_entity:pig:1f085b2d95484159a8c7f3ccdf0c2054>x")
    );
    assert!(matches!(
        hover("<hover:show_entity:pig:1f085b2d-9548-4159-a8c7-f3ccdf0c2054>x"),
        Some(HoverIr::Entity { uuid, name: None, .. }) if uuid == NOTCH
    ));
}

#[test]
fn nbt_takes_a_separator_and_interpret() {
    let piece = |input: &str| match &pieces(input, Mode::Runtime)[0] {
        Piece::Nbt {
            interpret,
            separator,
            ..
        } => (*interpret, separator.is_some()),
        other => panic!("{input:?}: expected an nbt piece, got {other:?}"),
    };
    assert_eq!(piece("<nbt:entity:'@s':Health>"), (false, false));
    assert_eq!(piece("<nbt:entity:'@s':Health:interpret>"), (true, false));
    assert_eq!(piece("<nbt:entity:'@s':Health:' '>"), (false, true));
    assert_eq!(
        piece("<nbt:entity:'@s':Health:' ':interpret>"),
        (true, true)
    );
}

#[test]
fn head_names_the_player_three_ways() {
    let head = |input: &str| match &pieces(input, Mode::Runtime)[0] {
        Piece::Head { player, hat, .. } => (player.clone(), *hat),
        other => panic!("{input:?}: expected a head, got {other:?}"),
    };
    assert_eq!(
        head("<head:Notch>"),
        (HeadIr::Name("Notch".to_string()), true)
    );
    assert_eq!(
        head("<head:1f085b2d-9548-4159-a8c7-f3ccdf0c2054>"),
        (HeadIr::Uuid(NOTCH), true)
    );
    assert_eq!(
        head("<head:'minecraft:textures/entity/steve'>"),
        (
            HeadIr::Texture("minecraft:textures/entity/steve".to_string()),
            true
        )
    );
    assert!(!head("<head:Notch:false>").1);
}

#[test]
fn sprite_defaults_to_the_block_atlas() {
    assert!(matches!(
        &pieces("<sprite:x>", Mode::Runtime)[0],
        Piece::Sprite { atlas, sprite, .. } if atlas == "minecraft:blocks" && sprite == "x"
    ));
    assert!(matches!(
        &pieces("<sprite:blocks:x>", Mode::Runtime)[0],
        Piece::Sprite { atlas, .. } if atlas == "blocks"
    ));
}

#[test]
fn nested_templates_carry_their_holes() {
    assert!(pieces("<selector:@a:'{name}'>", Mode::Runtime)[0].has_dyn());
    assert!(!pieces("<selector:@a:', '>", Mode::Runtime)[0].has_dyn());
    assert!(pieces("<nbt:entity:'@s':Health:'{sep}'>", Mode::Runtime)[0].has_dyn());
    assert!(
        pieces(
            "<hover:show_entity:pig:1f085b2d95484159a8c7f3ccdf0c2054:'{n}'>x",
            Mode::Runtime
        )[0]
        .has_dyn()
    );
    assert!(!pieces("<hover:show_item:'minecraft:stone':3>x", Mode::Runtime)[0].has_dyn());
}

fn static_bytes(input: &str) -> Vec<u8> {
    emit(&pieces(input, Mode::Runtime))
        .unwrap()
        .iter()
        .filter_map(|seg| match seg {
            NbtSeg::Bytes(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn holds(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn unresolved_content_becomes_its_own_segment() {
    let segs = emit(&pieces("<score:a:b/>", Mode::Runtime)).unwrap();
    assert!(
        segs.iter().any(|seg| matches!(
            seg,
            NbtSeg::Unresolved {
                what: Unresolved::Scoreboard,
                ..
            }
        )),
        "{segs:?}"
    );
}

#[test]
fn sprite_writes_the_atlas_only_when_it_differs() {
    assert!(holds(
        &static_bytes("<sprite:blocks:x/>"),
        &to_mutf8("atlas")
    ));
    assert!(!holds(&static_bytes("<sprite:x/>"), &to_mutf8("atlas")));
}

#[test]
fn show_item_writes_the_count_only_when_it_differs() {
    let one = static_bytes("<hover:show_item:'minecraft:stone'>x</hover>");
    assert!(holds(&one, &to_mutf8("show_item")));
    assert!(!holds(&one, &to_mutf8("count")));
    assert!(holds(
        &static_bytes("<hover:show_item:'minecraft:stone':3>x</hover>"),
        &to_mutf8("count")
    ));
}

#[test]
fn show_entity_writes_the_uuid_as_an_int_array() {
    let bytes =
        static_bytes("<hover:show_entity:pig:1f085b2d-9548-4159-a8c7-f3ccdf0c2054>x</hover>");
    let mut needle = to_mutf8("uuid");
    needle.extend_from_slice(&4i32.to_be_bytes());
    needle.extend_from_slice(&NOTCH);
    assert!(holds(&bytes, &needle));
}
