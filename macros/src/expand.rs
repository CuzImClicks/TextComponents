use proc_macro::TokenStream;
use proc_macro2::TokenStream as Ts;
use quote::{format_ident, quote};
use syn::LitStr;
use text_components_grammar::nbt::{EventKind, NbtSeg, SplicePos, emit, to_mutf8};
use text_components_grammar::{StrSeg, Style};

use crate::analysis::{
    Usage, positional_binds, reject_const_holes, reject_const_splice_in_hole_hover, spec_format,
};
use crate::codegen::Codegen;
use crate::hygiene::hygienic;
use crate::nbt::{Run, nbt_string_tokens, static_runs, write_formatted_tokens};
use crate::parse::{Input, checked_parse, emit_error_to_syn, precise_span};

pub(crate) fn text(input: TokenStream) -> TokenStream {
    let Input { fmt, args } = syn::parse_macro_input!(input as Input);
    let template = match checked_parse(&fmt, &args) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let pieces = template.pieces;
    if let Err(e) = reject_const_holes(&pieces, &fmt) {
        return e.to_compile_error().into();
    }
    let mut cg = Codegen::new(&fmt, &pieces);
    let binds = positional_binds(&args, &cg.uses);

    let body = cg.component_expr(&pieces);

    let statics = &cg.statics;
    hygienic(quote! {
        {
            #(#statics)*
            #(#binds)*
            #body
        }
    })
    .into()
}

fn unresolved_error(segs: &[NbtSeg], fmt: &LitStr) -> Option<syn::Error> {
    segs.iter().find_map(|seg| match seg {
        NbtSeg::Unresolved { what, range } => {
            let span = precise_span(fmt, *range).unwrap_or_else(|| fmt.span());
            Some(syn::Error::new(
                span,
                format!(
                    "`<{}>` needs resolution before it can be encoded, and text_nbt! produces \
                     bytes ready to send. Build the message with text! and resolve it, or resolve \
                     the component and pass it through a {{@}} hole",
                    what.tag()
                ),
            ))
        }
        _ => None,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "one match arm per tag/variant; splitting hides the shape"
)]
pub(crate) fn text_nbt(input: TokenStream) -> TokenStream {
    let Input { fmt, args } = syn::parse_macro_input!(input as Input);
    let template = match checked_parse(&fmt, &args) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let pieces = template.pieces;
    if let Err(e) = reject_const_splice_in_hole_hover(&pieces, &fmt) {
        return e.to_compile_error().into();
    }
    let segs = match emit(&pieces) {
        Ok(segs) => segs,
        Err(e) => {
            return emit_error_to_syn(&e, &fmt).to_compile_error().into();
        }
    };

    if let Some(e) = unresolved_error(&segs, &fmt) {
        return e.to_compile_error().into();
    }

    let runs = static_runs(&segs);

    if let [Run::Static(run)] = &runs[..] {
        let mut cg = Codegen::new(&fmt, &pieces);
        let id = format_ident!("__TCM_NBT");
        let (items, _) = cg.static_run(run, &id);
        return hygienic(quote! {
            {
                #(#items)*
                ::text_components::EncodedComponent::from_static(#id)
            }
        })
        .into();
    }

    let mut cg = Codegen::new(&fmt, &pieces);
    let mut statics: Vec<Ts> = Vec::new();
    let mut writes: Vec<Ts> = Vec::new();
    let mut static_len = 0usize;
    let mut cap_terms: Vec<Ts> = Vec::new();
    let mut hole_n = 0usize;
    let mut static_n = 0usize;

    for run in &runs {
        let seg = match run {
            Run::Static(run) => {
                let id = format_ident!("__TCM_NB{static_n}");
                static_n += 1;
                let (items, len) = cg.static_run(run, &id);
                statics.extend(items);
                cap_terms.push(quote!(+ #len));
                writes.push(quote! { __tcm_buf.extend_from_slice(#id); });
                continue;
            }
            Run::Runtime(seg) => seg,
        };
        match seg {
            NbtSeg::Bytes(_) | NbtSeg::ConstComponent { .. } | NbtSeg::ConstText(_) => {
                unreachable!("byte runs and const splices are folded into statics")
            }
            NbtSeg::Unresolved { .. } => {
                unreachable!("unresolved content is rejected before codegen")
            }
            NbtSeg::Hole(arg, spec) => {
                let id = format_ident!("__tcm_a{hole_n}");
                hole_n += 1;
                let expr = cg.arg_expr(arg, Usage::Borrow);
                let fmt = spec_format(spec.as_deref());
                cap_terms.push(quote!(+ 2 + 16));
                writes.push(quote! { let #id = #expr; });
                writes.push(nbt_string_tokens(write_formatted_tokens(&fmt, &id)));
            }
            NbtSeg::StrSplice(parts) => {
                let mut inner: Vec<Ts> = Vec::new();
                for part in parts {
                    match part {
                        StrSeg::Lit(lit) => {
                            let bytes = to_mutf8(lit);
                            static_len += bytes.len();
                            inner.push(quote! {
                                __tcm_buf.extend_from_slice(&[#(#bytes),*]);
                            });
                        }
                        StrSeg::Hole(arg, spec) => {
                            let id = format_ident!("__tcm_a{hole_n}");
                            hole_n += 1;
                            let expr = cg.arg_expr(arg, Usage::Borrow);
                            let fmt = spec_format(spec.as_deref());
                            writes.push(quote! { let #id = #expr; });
                            cap_terms.push(quote!(+ 16));
                            inner.push(write_formatted_tokens(&fmt, &id));
                        }
                    }
                }
                cap_terms.push(quote!(+ 2));
                writes.push(nbt_string_tokens(quote!(#(#inner)*)));
            }
            NbtSeg::BoolHole(arg) => {
                let id = format_ident!("__tcm_a{hole_n}");
                hole_n += 1;
                let expr = cg.arg_expr(arg, Usage::Consume);
                writes.push(quote! {
                    let #id: bool = #expr;
                });
                cap_terms.push(quote!(+ 1));
                writes.push(quote! { __tcm_buf.push(#id as u8); });
            }
            NbtSeg::ColorHole(arg) => {
                let id = format_ident!("__tcm_a{hole_n}");
                hole_n += 1;
                let expr = cg.arg_expr(arg, Usage::Borrow);
                writes.push(quote! {
                    let #id: &::text_components::format::Color =
                        ::core::borrow::Borrow::borrow(#expr);
                });
                cap_terms.push(quote!(+ 2 + 12));
                writes.push(quote! {
                    ::text_components::__private::write_color(&mut __tcm_buf, #id);
                });
            }
            NbtSeg::EventHole { arg, kind } => {
                let id = format_ident!("__tcm_a{hole_n}");
                hole_n += 1;
                let expr = cg.arg_expr(arg, Usage::Consume);
                let ty = match kind {
                    EventKind::Hover => quote!(::text_components::interactivity::HoverEvent),
                    EventKind::Click => quote!(::text_components::interactivity::ClickEvent),
                };
                writes.push(quote! {
                    let #id: #ty = ::core::convert::Into::into(#expr);
                });
                cap_terms.push(quote!(+ 64));
                // the entry header is already in the static run, so no tag type byte here
                writes.push(quote! {
                    {
                        let __tcm_tag = #id.to_codec_nbt();
                        let mut __tcm_tmp = ::std::vec::Vec::with_capacity(128);
                        __tcm_tag.write(&mut __tcm_tmp);
                        __tcm_buf.extend_from_slice(&__tcm_tmp[1..]);
                    }
                });
            }
            NbtSeg::LangKey { arg, arity } => {
                let id = format_ident!("__tcm_a{hole_n}");
                hole_n += 1;
                let expr = cg.arg_expr(arg, Usage::Borrow);
                // borrowing as Translation<arity> makes an arity mismatch a type error
                writes.push(quote! {
                    let #id: &::text_components::translation::Translation<#arity> =
                        ::core::borrow::Borrow::borrow(#expr);
                });
                cap_terms.push(quote!(+ 2 + 48));
                writes.push(quote! {
                    ::text_components::__private::write_mutf8(&mut __tcm_buf, #id.0);
                });
            }
            NbtSeg::Component { arg, style, at } => {
                let id = format_ident!("__tcm_a{hole_n}");
                hole_n += 1;
                if let Some(hint) = cg.encoded_hint(arg) {
                    writes.push(hint);
                }
                let expr = cg.arg_expr(arg, Usage::Consume);
                writes.push(quote! { let #id = #expr; });
                let style_arg = if **style == Style::default() {
                    quote!(::core::option::Option::None)
                } else {
                    let style_id = format_ident!("__tcm_st{hole_n}");
                    let fills = cg.fill_tokens(style, &style_id);
                    writes.push(quote! {
                        let mut #style_id: ::text_components::TextComponent =
                            ::text_components::TextComponent::plain("");
                        #(#fills)*
                    });
                    quote!(::core::option::Option::Some(&#style_id))
                };
                cap_terms.push(quote!(+ 64));
                writes.push(match at {
                    SplicePos::Root => quote! {
                        #id.splice_root(&mut __tcm_buf, #style_arg);
                    },
                    SplicePos::Elem => quote! {
                        #id.splice_elem(&mut __tcm_buf, #style_arg);
                    },
                    SplicePos::Entry(name) => quote! {
                        #id.splice_entry(&mut __tcm_buf, #name, #style_arg);
                    },
                });
            }
        }
    }

    let cg_statics = &cg.statics;
    let prologue = positional_binds(&args, &cg.uses);
    hygienic(quote! {
        {
            use ::text_components::SpliceComponent as _;
            use ::text_components::__private::AlreadyEncoded as _;
            #(#statics)*
            #(#cg_statics)*
            #(#prologue)*
            let mut __tcm_buf: ::std::vec::Vec<u8> =
                ::std::vec::Vec::with_capacity(#static_len #(#cap_terms)*);
            #(#writes)*
            ::text_components::EncodedComponent::from_vec(__tcm_buf)
        }
    })
    .into()
}
