//! Malformed input errors instead of panicking, and equivalent spellings parse identically.
#![expect(clippy::unwrap_used, reason = "tests unwrap known-good input")]

use text_components_grammar::nbt::emit;
use text_components_grammar::{
    ColorIr, HoleArg, HoleKind, HoverIr, Mode, Piece, StrSeg, Style, parse,
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
