use proc_macro2::{Ident, TokenStream as Ts};
use quote::quote;
use text_components_grammar::nbt::NbtSeg;

pub(crate) enum Run<'a> {
    Static(Vec<&'a NbtSeg>),
    Runtime(&'a NbtSeg),
}

pub(crate) fn static_runs(segs: &[NbtSeg]) -> Vec<Run<'_>> {
    let mut runs: Vec<Run> = Vec::new();
    for seg in segs {
        match seg {
            NbtSeg::Bytes(_) | NbtSeg::ConstComponent { .. } | NbtSeg::ConstText(_) => {
                match runs.last_mut() {
                    Some(Run::Static(run)) => run.push(seg),
                    _ => runs.push(Run::Static(vec![seg])),
                }
            }
            other => runs.push(Run::Runtime(other)),
        }
    }
    runs
}

pub(crate) fn nbt_string_tokens(body: Ts) -> Ts {
    quote! {
        {
            let __tcm_at = __tcm_buf.len();
            __tcm_buf.extend_from_slice(&[0, 0]);
            #body
            let __tcm_n = ::text_components::__private::nbt_len(__tcm_buf.len() - __tcm_at - 2);
            __tcm_buf[__tcm_at..__tcm_at + 2].copy_from_slice(&__tcm_n.to_be_bytes());
        }
    }
}

pub(crate) fn write_formatted_tokens(fmt: &str, id: &Ident) -> Ts {
    quote! {
        let _ = ::core::fmt::Write::write_fmt(
            &mut ::text_components::__private::Mutf8Fmt(&mut __tcm_buf),
            ::core::format_args!(#fmt, #id),
        );
    }
}
