use proc_macro2::{Ident, TokenStream as Ts};
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Expr, LitStr};
use text_components_grammar::{
    BoolIr, ClickIr, ColorIr, HoleArg, HoleKind, HoverIr, LangKey, Piece, StrSeg, Style,
};

use crate::parse::precise_span;

#[derive(PartialEq, Eq, Hash)]
pub(crate) enum HoleKey {
    Positional(usize),
    Named(String),
}

pub(crate) fn hole_key(arg: &HoleArg) -> HoleKey {
    match arg {
        HoleArg::Positional(i) => HoleKey::Positional(*i),
        HoleArg::Named { name, .. } => HoleKey::Named(name.clone()),
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Usage {
    Consume,
    Borrow,
    Const,
    ConstText,
}

pub(crate) fn positional_ident(i: usize) -> Ident {
    format_ident!("__tcm_p{i}")
}

/// Binds every positional argument exactly once.
pub(crate) fn positional_binds(args: &[Expr], uses: &HashMap<HoleKey, Uses>) -> Vec<Ts> {
    args.iter()
        .enumerate()
        .map(|(i, e)| {
            let id = positional_ident(i);
            if consumes_by_value(uses, &HoleKey::Positional(i)) {
                quote!(let #id = #e;)
            } else {
                quote!(let #id = &(#e);)
            }
        })
        .collect()
}

pub(crate) fn consumes_by_value(uses: &HashMap<HoleKey, Uses>, key: &HoleKey) -> bool {
    uses.get(key).is_some_and(|u| u.consumes > 0)
}

#[derive(Default)]
pub(crate) struct Uses {
    pub(crate) total: usize,
    pub(crate) consumes: usize,
}

pub(crate) fn walk_holes(pieces: &[Piece], visit: &mut impl FnMut(&HoleArg, Usage)) {
    for piece in pieces {
        match piece {
            Piece::Hole { arg, kind, .. } => visit(
                arg,
                match kind {
                    HoleKind::Component => Usage::Consume,
                    HoleKind::ConstComponent => Usage::Const,
                    HoleKind::ConstText => Usage::ConstText,
                    HoleKind::Text => Usage::Borrow,
                },
            ),
            Piece::Lang {
                key,
                fallback,
                args,
                ..
            } => {
                if let LangKey::Dyn(arg) = key {
                    visit(arg, Usage::Borrow);
                }
                if let Some(segs) = fallback {
                    walk_segs(segs, visit);
                }
                for arg in args {
                    walk_holes(arg, visit);
                }
            }
            Piece::Selector { separator, .. } | Piece::Nbt { separator, .. } => {
                if let Some(separator) = separator {
                    walk_holes(separator, visit);
                }
            }
            Piece::Text { .. }
            | Piece::Keybind { .. }
            | Piece::Score { .. }
            | Piece::Sprite { .. }
            | Piece::Head { .. } => {}
        }
        walk_style(piece.style(), visit);
    }
}

pub(crate) fn walk_style(style: &Style, visit: &mut impl FnMut(&HoleArg, Usage)) {
    if let Some(ColorIr::Dyn(arg)) = &style.color {
        visit(arg, Usage::Borrow);
    }
    for (_, flag) in style.flags() {
        if let Some(BoolIr::Dyn(arg)) = flag {
            visit(arg, Usage::Consume);
        }
    }
    if let Some(segs) = &style.insertion {
        walk_segs(segs, visit);
    }
    match &style.click {
        Some(ClickIr::Dyn(arg)) => visit(arg, Usage::Consume),
        Some(ClickIr::Action(_, segs)) => walk_segs(segs, visit),
        None => {}
    }
    match &style.hover {
        Some(HoverIr::Dyn(arg)) => visit(arg, Usage::Consume),
        Some(HoverIr::Text(pieces)) => walk_holes(pieces, visit),
        Some(HoverIr::Entity { name, .. }) => {
            if let Some(name) = name {
                walk_holes(name, visit);
            }
        }
        Some(HoverIr::Item { .. }) | None => {}
    }
}

pub(crate) fn walk_segs(segs: &[StrSeg], visit: &mut impl FnMut(&HoleArg, Usage)) {
    for seg in segs {
        if let StrSeg::Hole(arg, _) = seg {
            visit(arg, Usage::Borrow);
        }
    }
}

pub(crate) fn count_uses(pieces: &[Piece]) -> HashMap<HoleKey, Uses> {
    let mut counts: HashMap<HoleKey, Uses> = HashMap::new();
    walk_holes(pieces, &mut |arg, usage| {
        let entry = counts.entry(hole_key(arg)).or_default();
        match usage {
            Usage::Consume => {
                entry.total += 1;
                entry.consumes += 1;
            }
            Usage::Borrow => entry.total += 1,
            // consts are named directly at their use sites: no binding, no move
            Usage::Const | Usage::ConstText => {}
        }
    });
    counts
}

pub(crate) fn reject_const_holes(pieces: &[Piece], lit: &LitStr) -> Result<(), syn::Error> {
    let mut failed = None;
    walk_holes(pieces, &mut |arg, usage| {
        if failed.is_none()
            && matches!(usage, Usage::Const)
            && let HoleArg::Named { range, .. } = arg
        {
            let span = precise_span(lit, *range).unwrap_or_else(|| lit.span());
            failed = Some(syn::Error::new(
                span,
                "`{@const …}` only works in text_nbt!. In text! use a plain {@NAME} hole",
            ));
        }
    });
    match failed {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Visits every piece, including those nested in hover markup and translation arguments.
fn walk_pieces(pieces: &[Piece], visit: &mut impl FnMut(&Piece)) {
    for piece in pieces {
        visit(piece);
        if let Piece::Lang { args, .. } = piece {
            for arg in args {
                walk_pieces(arg, visit);
            }
        }
        if let Piece::Selector { separator, .. } | Piece::Nbt { separator, .. } = piece
            && let Some(separator) = separator
        {
            walk_pieces(separator, visit);
        }
        match &piece.style().hover {
            Some(HoverIr::Text(inner)) => walk_pieces(inner, visit),
            Some(HoverIr::Entity {
                name: Some(name), ..
            }) => walk_pieces(name, visit),
            _ => {}
        }
    }
}

/// The span of the first `{@const …}` hole in `pieces`.
fn const_splice_span(pieces: &[Piece], lit: &LitStr) -> Option<proc_macro2::Span> {
    let mut found = None;
    walk_holes(pieces, &mut |arg, usage| {
        if found.is_none()
            && matches!(usage, Usage::Const)
            && let HoleArg::Named { range, .. } = arg
        {
            found = Some(precise_span(lit, *range).unwrap_or_else(|| lit.span()));
        }
    });
    found
}

/// A `{@const …}` splice is encoded bytes, but the hover of a `{@…}` component hole is built as a
/// [`HoverEvent`] at runtime, which holds a component instead.
pub(crate) fn reject_const_splice_in_hole_hover(
    pieces: &[Piece],
    lit: &LitStr,
) -> Result<(), syn::Error> {
    let mut failed = None;
    walk_pieces(pieces, &mut |piece| {
        if failed.is_none()
            && let Piece::Hole {
                kind: HoleKind::Component,
                style,
                ..
            } = piece
            && let Some(HoverIr::Text(inner)) = &style.hover
            && let Some(span) = const_splice_span(inner, lit)
        {
            failed = Some(syn::Error::new(
                span,
                "`{@const …}` cannot go in the hover of a `{@…}` hole, because that hover is \
                 built as a component at runtime and a const splice is already-encoded bytes. \
                 Write the hover as `{const NAME}` with a `&'static str`, or take the `{@…}` \
                 hole out so the whole template stays const",
            ));
        }
    });
    match failed {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

pub(crate) fn check_hole_names(pieces: &[Piece], lit: &LitStr) -> Result<(), syn::Error> {
    let mut failed = None;
    walk_holes(pieces, &mut |arg, _| {
        if failed.is_none()
            && let HoleArg::Named { name, range } = arg
            && syn::parse_str::<Ident>(name).is_err()
        {
            let span = precise_span(lit, *range).unwrap_or_else(|| lit.span());
            failed = Some(syn::Error::new(
                span,
                format!("`{name}` is not a name a variable can have"),
            ));
        }
    });
    match failed {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

pub(crate) fn spec_format(spec: Option<&str>) -> String {
    match spec {
        None => "{}".to_string(),
        Some(spec) => format!("{{:{spec}}}"),
    }
}

pub(crate) fn is_screaming_snake(name: &str) -> bool {
    name.chars().any(char::is_uppercase)
        && name
            .chars()
            .all(|c| c.is_uppercase() || c.is_numeric() || c == '_')
}
