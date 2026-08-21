use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Expr, LitStr, Token};
use text_components_grammar::nbt::EmitError;
use text_components_grammar::{self as grammar, HoleArg, Mode, ParseError, Piece};

use crate::analysis::check_hole_names;

pub(crate) struct Input {
    pub(crate) fmt: LitStr,
    pub(crate) args: Vec<Expr>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fmt: LitStr = input.parse()?;
        let mut args = Vec::new();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            args.push(input.parse()?);
        }
        Ok(Input { fmt, args })
    }
}

pub(crate) fn precise_span(lit: &LitStr, (start, end): (usize, usize)) -> Option<Span> {
    let token = lit.token();
    let repr = token.to_string();
    let (body_start, map) = value_to_raw(&repr)?;
    let last = map.len() - 1;
    let end = end.clamp((start + 1).min(last), last);
    let (bs, be) = (*map.get(start)?, *map.get(end)?);
    token.subspan(body_start + bs..body_start + be)
}

/// The body's offset inside `repr`, and each value char's byte offset in it.
fn value_to_raw(repr: &str) -> Option<(usize, Vec<usize>)> {
    let (body_start, body) = if let Some(rest) = repr.strip_prefix('"') {
        (1, rest.strip_suffix('"')?)
    } else {
        let rest = repr.strip_prefix('r')?;
        let hashes = rest.bytes().take_while(|&b| b == b'#').count();
        let rest = rest[hashes..].strip_prefix('"')?;
        let body = rest
            .strip_suffix(&repr[repr.len() - hashes..])?
            .strip_suffix('"')?;
        (1 + hashes + 1, body)
    };
    let mut map = Vec::with_capacity(body.len() + 1);
    if body_start == 1 {
        rustc_literal_escaper::unescape_str(body, |range, result| {
            if result.is_ok() {
                map.push(range.start);
            }
        });
    } else {
        map.extend(body.char_indices().map(|(i, _)| i));
    }
    map.push(body.len());
    Some((body_start, map))
}

pub(crate) fn parse_error_to_syn(err: &ParseError, lit: &LitStr) -> syn::Error {
    let span = precise_span(lit, err.range).unwrap_or_else(|| lit.span());
    syn::Error::new(span, err.render(&lit.value()))
}

pub(crate) fn emit_error_to_syn(err: &EmitError, lit: &LitStr) -> syn::Error {
    match err.range {
        Some(range) => parse_error_to_syn(
            &ParseError {
                range,
                message: err.message.clone(),
                help: None,
            },
            lit,
        ),
        None => syn::Error::new(lit.span(), err.message.clone()),
    }
}

pub(crate) fn checked_parse(fmt: &LitStr, args: &[Expr]) -> Result<grammar::Template, TokenStream> {
    let template = match grammar::parse(&fmt.value(), Mode::Macro) {
        Ok(t) => t,
        Err(e) => return Err(parse_error_to_syn(&e, fmt).to_compile_error().into()),
    };
    if let Err(e) = check_hole_names(&template.pieces, fmt) {
        return Err(e.to_compile_error().into());
    }
    if args.len() > template.positional_count {
        return Err(syn::Error::new(
            args[template.positional_count].span(),
            format!(
                "unexpected argument — the format string only has {} positional hole(s)",
                template.positional_count
            ),
        )
        .to_compile_error()
        .into());
    }
    if args.len() < template.positional_count {
        let unfilled = template
            .pieces
            .iter()
            .find_map(|p| match p {
                Piece::Hole {
                    arg: HoleArg::Positional(i),
                    range,
                    ..
                } if *i >= args.len() => Some(*range),
                _ => None,
            })
            .unwrap_or((0, 0));
        let err = ParseError {
            range: unfilled,
            message: format!(
                "this hole has no argument — {} hole(s) but only {} argument(s)",
                template.positional_count,
                args.len()
            ),
            help: Some("add an argument after the format string, like format!()".into()),
        };
        return Err(parse_error_to_syn(&err, fmt).to_compile_error().into());
    }
    Ok(template)
}
