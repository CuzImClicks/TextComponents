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
            Piece::Text { .. } | Piece::Keybind { .. } => {}
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
        None => {}
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
            // const splices are folded at compile time: no binding, no move
            Usage::Const => {}
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
