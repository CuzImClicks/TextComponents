use proc_macro2::{Ident, TokenStream as Ts};
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashMap;
use syn::LitStr;
use text_components_grammar::nbt::{NbtSeg, SplicePos};
use text_components_grammar::{
    self as grammar, BoolIr, ClickIr, ClickKind, ColorIr, HoleArg, HoleKind, HoverIr, LangKey,
    Piece, StrSeg, Style,
};

use crate::analysis::{
    HoleKey, Usage, Uses, consumes_by_value, count_uses, hole_key, is_screaming_snake,
    positional_ident, spec_format,
};
use crate::parse::precise_span;

pub(crate) struct Codegen {
    lit: LitStr,
    pub(crate) statics: Vec<Ts>,
    counter: usize,
    /// Counted down as sites are generated; the last use in expansion order moves.
    pub(crate) uses: HashMap<HoleKey, Uses>,
}

impl Codegen {
    pub(crate) fn new(lit: &LitStr, pieces: &[Piece]) -> Self {
        Codegen {
            lit: lit.clone(),
            statics: Vec::new(),
            counter: 0,
            uses: count_uses(pieces),
        }
    }

    pub(crate) fn fresh(&mut self, prefix: &str) -> Ident {
        let id = format_ident!("__TCM_{}{}", prefix, self.counter);
        self.counter += 1;
        id
    }

    pub(crate) fn arg_ident(&self, arg: &HoleArg) -> Ident {
        match arg {
            HoleArg::Positional(i) => positional_ident(*i),
            HoleArg::Named { name, range } => Ident::new(
                name,
                precise_span(&self.lit, *range).unwrap_or_else(|| self.lit.span()),
            ),
        }
    }

    pub(crate) fn arg_expr(&mut self, arg: &HoleArg, usage: Usage) -> Ts {
        let id = self.arg_ident(arg);
        // the `&` must carry the hole's span too, or rustc spans the whole invocation
        let span = id.span();
        let key = hole_key(arg);
        let last = match self.uses.get_mut(&key) {
            Some(uses) if uses.total > 0 => {
                uses.total -= 1;
                uses.total == 0
            }
            _ => false,
        };
        let by_ref = matches!(arg, HoleArg::Positional(_)) && !consumes_by_value(&self.uses, &key);
        match usage {
            Usage::Borrow if by_ref => quote_spanned!(span=> #id),
            Usage::Borrow => quote_spanned!(span=> &#id),
            Usage::Consume if last => quote!(#id),
            Usage::Consume => quote!(::core::clone::Clone::clone(&#id)),
            Usage::Const => unreachable!("a const splice has no expression to evaluate"),
        }
    }

    pub(crate) fn static_run(&mut self, run: &[&NbtSeg], id: &Ident) -> (Vec<Ts>, Ts) {
        if let [NbtSeg::Bytes(bytes)] = run {
            let len = bytes.len();
            return (
                vec![quote! { static #id: &[u8] = &[#(#bytes),*]; }],
                quote!(#len),
            );
        }
        let mut items: Vec<Ts> = Vec::new();
        let mut parts: Vec<Ts> = Vec::new();
        for seg in run {
            match seg {
                NbtSeg::Bytes(bytes) => parts
                    .push(quote!(::text_components::__private::SplicePart::Raw(&[#(#bytes),*]))),
                NbtSeg::ConstComponent { arg, at } => {
                    let name = self.arg_ident(arg);
                    let span = name.span();
                    let const_id = self.fresh("SPLICEREF");
                    let bytes_id = self.fresh("SPLICE");
                    // borrowing the const keeps the value out of const-eval's drop rules
                    items.push(quote_spanned! {span=>
                        const #const_id: &::text_components::EncodedComponent = &#name;
                        const #bytes_id: &[u8] = #const_id.as_bytes();
                    });
                    parts.push(match at {
                        SplicePos::Root => {
                            quote!(::text_components::__private::SplicePart::Root(#bytes_id))
                        }
                        SplicePos::Elem => {
                            quote!(::text_components::__private::SplicePart::Elem(#bytes_id))
                        }
                        SplicePos::Entry(entry) => {
                            quote!(::text_components::__private::SplicePart::Entry(#entry, #bytes_id))
                        }
                    });
                }
                _ => unreachable!("a static run holds bytes and const splices"),
            }
        }
        let parts_id = self.fresh("SPLICEPARTS");
        let len_id = self.fresh("SPLICELEN");
        let array_id = self.fresh("SPLICED");
        items.push(quote! {
            const #parts_id: &[::text_components::__private::SplicePart<'static>] = &[#(#parts),*];
            const #len_id: usize = ::text_components::__private::splice_len(#parts_id);
            const #array_id: [u8; #len_id] =
                ::text_components::__private::splice_bytes(#parts_id);
            static #id: &[u8] = &#array_id;
        });
        (items, quote!(#len_id))
    }

    pub(crate) fn encoded_hint(&self, arg: &HoleArg) -> Option<Ts> {
        let HoleArg::Named { name, .. } = arg else {
            return None;
        };
        if !is_screaming_snake(name) {
            return None;
        }
        let id = self.arg_ident(arg);
        let span = id.span();
        Some(quote_spanned!(span=> #id.already_encoded();))
    }

    pub(crate) fn dyn_color_tokens(&mut self, arg: &HoleArg) -> Ts {
        let e = self.arg_expr(arg, Usage::Borrow);
        quote!(::core::clone::Clone::clone(
            ::core::borrow::Borrow::<::text_components::format::Color>::borrow(#e)
        ))
    }

    pub(crate) fn color_tokens(color: &ColorIr) -> Ts {
        match color {
            ColorIr::Dyn(_) => unreachable!("dynamic color in static-only codegen"),
            ColorIr::Named(name) => {
                let variant = format_ident!(
                    "{}",
                    name.split('_')
                        .map(|w| {
                            let mut c = w.chars();
                            c.next()
                                .expect("color name words are never empty")
                                .to_uppercase()
                                .collect::<String>()
                                + c.as_str()
                        })
                        .collect::<String>()
                );
                quote!(::text_components::format::Color::#variant)
            }
            ColorIr::Rgb(r, g, b) => {
                quote!(::text_components::format::Color::Rgb(#r, #g, #b))
            }
        }
    }

    pub(crate) fn format_tokens(style: &Style) -> Ts {
        let opt = |v: &Option<BoolIr>| match v {
            Some(BoolIr::Const(b)) => quote!(::core::option::Option::Some(#b)),
            Some(BoolIr::Dyn(_)) => unreachable!("dynamic flag in static-only codegen"),
            None => quote!(::core::option::Option::None),
        };
        let color = if let Some(c) = &style.color {
            let c = Self::color_tokens(c);
            quote!(::core::option::Option::Some(#c))
        } else {
            quote!(::core::option::Option::None)
        };
        let (bold, italic, underlined, strikethrough, obfuscated) = (
            opt(&style.bold),
            opt(&style.italic),
            opt(&style.underlined),
            opt(&style.strikethrough),
            opt(&style.obfuscated),
        );
        let font = if let Some(font) = &style.font {
            quote!(::core::option::Option::Some(::std::borrow::Cow::Borrowed(#font)))
        } else {
            quote!(::core::option::Option::None)
        };
        let shadow = if let Some(shadow) = style.shadow_color {
            quote!(::core::option::Option::Some(#shadow))
        } else {
            quote!(::core::option::Option::None)
        };
        quote!(::text_components::format::Format {
            color: #color,
            font: #font,
            bold: #bold,
            italic: #italic,
            underlined: #underlined,
            strikethrough: #strikethrough,
            obfuscated: #obfuscated,
            shadow_color: #shadow,
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match arm per tag/variant; splitting hides the shape"
    )]
    pub(crate) fn interactivity_tokens(&mut self, style: &Style) -> Ts {
        let insertion = match &style.insertion {
            None => quote!(::core::option::Option::None),
            Some(segs) => {
                if let Some(value) = StrSeg::join_lits(segs) {
                    quote!(::core::option::Option::Some(::std::borrow::Cow::Borrowed(#value)))
                } else {
                    let value = self.str_splice_expr(segs);
                    quote!(::core::option::Option::Some(::std::borrow::Cow::Owned(#value)))
                }
            }
        };
        let click = match &style.click {
            None => quote!(::core::option::Option::None),
            Some(ClickIr::Dyn(arg)) => {
                let e = self.arg_expr(arg, Usage::Consume);
                quote!(::core::option::Option::Some(
                    ::text_components::interactivity::MaybeStatic::Owned(::std::boxed::Box::new(
                        ::core::convert::Into::<
                            ::text_components::interactivity::ClickEvent,
                        >::into(#e),
                    ))
                ))
            }
            Some(ClickIr::Action(kind, segs)) => {
                let (variant, field) = match kind {
                    ClickKind::OpenUrl => (quote!(OpenUrl), quote!(url)),
                    ClickKind::RunCommand => (quote!(RunCommand), quote!(command)),
                    ClickKind::SuggestCommand => (quote!(SuggestCommand), quote!(command)),
                    ClickKind::CopyToClipboard => (quote!(CopyToClipboard), quote!(value)),
                    ClickKind::ChangePage(_) => (quote!(ChangePage), quote!(page)),
                };
                if let ClickKind::ChangePage(page) = kind {
                    let event = quote!(::text_components::interactivity::ClickEvent::ChangePage {
                        page: #page
                    });
                    let id = self.fresh("CLICK");
                    self.statics.push(quote! {
                        static #id: ::text_components::interactivity::ClickEvent = #event;
                    });
                    quote!(::core::option::Option::Some(
                        ::text_components::interactivity::MaybeStatic::Static(&#id)
                    ))
                } else if let Some(value) = StrSeg::join_lits(segs) {
                    let event = quote!(::text_components::interactivity::ClickEvent::#variant {
                        #field: ::std::borrow::Cow::Borrowed(#value)
                    });
                    let id = self.fresh("CLICK");
                    self.statics.push(quote! {
                        static #id: ::text_components::interactivity::ClickEvent = #event;
                    });
                    quote!(::core::option::Option::Some(
                        ::text_components::interactivity::MaybeStatic::Static(&#id)
                    ))
                } else {
                    let value = self.str_splice_expr(segs);
                    quote!(::core::option::Option::Some(
                        ::text_components::interactivity::MaybeStatic::Owned(
                            ::std::boxed::Box::new(
                                ::text_components::interactivity::ClickEvent::#variant {
                                    #field: ::std::borrow::Cow::Owned(#value),
                                }
                            )
                        )
                    ))
                }
            }
        };
        let hover = match &style.hover {
            None => quote!(::core::option::Option::None),
            Some(HoverIr::Dyn(arg)) => {
                let e = self.arg_expr(arg, Usage::Consume);
                quote!(::core::option::Option::Some(
                    ::text_components::interactivity::MaybeStatic::Owned(::std::boxed::Box::new(
                        ::core::convert::Into::<
                            ::text_components::interactivity::HoverEvent,
                        >::into(#e),
                    ))
                ))
            }
            Some(HoverIr::Text(pieces)) if !grammar::pieces_have_dyn(pieces) => {
                let value = self.static_component_tokens(pieces);
                let val_id = self.fresh("HOVERTEXT");
                let ev_id = self.fresh("HOVER");
                self.statics.push(quote! {
                    static #val_id: ::text_components::TextComponent = #value;
                    static #ev_id: ::text_components::interactivity::HoverEvent =
                        ::text_components::interactivity::HoverEvent::ShowText {
                            value: ::text_components::interactivity::MaybeStatic::Static(&#val_id),
                        };
                });
                quote!(::core::option::Option::Some(
                    ::text_components::interactivity::MaybeStatic::Static(&#ev_id)
                ))
            }
            Some(HoverIr::Text(pieces)) => {
                let value = self.component_expr(pieces);
                quote!(::core::option::Option::Some(
                    ::text_components::interactivity::MaybeStatic::Owned(::std::boxed::Box::new(
                        ::text_components::interactivity::HoverEvent::ShowText {
                            value: ::text_components::interactivity::MaybeStatic::Owned(
                                ::std::boxed::Box::new(#value),
                            ),
                        },
                    ))
                ))
            }
        };
        quote!(::text_components::interactivity::Interactivity {
            insertion: #insertion,
            click: #click,
            hover: #hover,
        })
    }

    pub(crate) fn str_splice_expr(&mut self, segs: &[StrSeg]) -> Ts {
        let parts: Vec<Ts> = segs
            .iter()
            .map(|seg| match seg {
                StrSeg::Lit(lit) => quote!(__tcm_s.push_str(#lit);),
                StrSeg::Hole(arg, spec) => {
                    let e = self.arg_expr(arg, Usage::Borrow);
                    let fmt = spec_format(spec.as_deref());
                    quote!(let _ = ::core::fmt::Write::write_fmt(
                        &mut __tcm_s, ::core::format_args!(#fmt, #e));)
                }
            })
            .collect();
        quote!({
            let mut __tcm_s = ::std::string::String::new();
            #(#parts)*
            __tcm_s
        })
    }

    pub(crate) fn leaf_tokens(&mut self, text: &str, style: &Style) -> Ts {
        let content = quote!(::text_components::content::Content::Text {
            text: ::std::borrow::Cow::Borrowed(#text),
        });
        self.content_leaf_tokens(content, style)
    }

    pub(crate) fn keybind_tokens(&mut self, key: &str, style: &Style) -> Ts {
        let content = quote!(::text_components::content::Content::Keybind {
            keybind: ::std::borrow::Cow::Borrowed(#key),
        });
        self.content_leaf_tokens(content, style)
    }

    pub(crate) fn static_leaf_tokens(&mut self, piece: &Piece, style: &Style) -> Ts {
        match piece {
            Piece::Keybind { key, .. } => self.keybind_tokens(key, style),
            Piece::Text { text, .. } => self.leaf_tokens(text, style),
            Piece::Lang {
                key: LangKey::Lit(key),
                args,
                ..
            } => {
                let args_expr = if args.is_empty() {
                    quote!(::text_components::Args::None)
                } else {
                    let n = args.len();
                    let elems: Vec<Ts> = args
                        .iter()
                        .map(|arg| self.static_component_tokens(arg))
                        .collect();
                    let args_id = self.fresh("ARGS");
                    self.statics.push(quote! {
                        static #args_id: [::text_components::TextComponent; #n] = [#(#elems),*];
                    });
                    quote!(::text_components::Args::Static(&#args_id))
                };
                let content = quote!(::text_components::content::Content::Translate(
                    ::text_components::translation::TranslatedMessage {
                        key: ::std::borrow::Cow::Borrowed(#key),
                        fallback: ::core::option::Option::None,
                        args: #args_expr,
                    }
                ));
                self.content_leaf_tokens(content, style)
            }
            Piece::Lang { .. } => unreachable!("dynamic <lang> pieces are never static leaves"),
            Piece::Hole { .. } => unreachable!("holes are not leaves"),
        }
    }

    pub(crate) fn content_leaf_tokens(&mut self, content: Ts, style: &Style) -> Ts {
        let format = Self::format_tokens(style);
        let interactivity = self.interactivity_tokens(style);
        quote!(::text_components::TextComponent {
            content: #content,
            children: ::std::borrow::Cow::Borrowed(&[]),
            format: #format,
            interactions: #interactivity,
        })
    }

    pub(crate) fn fill_tokens(&mut self, style: &Style, var: &Ident) -> Vec<Ts> {
        let mut fills: Vec<Ts> = Vec::new();
        if let Some(color) = &style.color {
            let c = match color {
                ColorIr::Dyn(arg) => self.dyn_color_tokens(arg),
                other => Self::color_tokens(other),
            };
            fills.push(quote! {
                if #var.format.color.is_none() {
                    #var.format.color = ::core::option::Option::Some(#c);
                }
            });
        }
        for (field, value) in [
            ("bold", &style.bold),
            ("italic", &style.italic),
            ("underlined", &style.underlined),
            ("strikethrough", &style.strikethrough),
            ("obfuscated", &style.obfuscated),
        ] {
            if let Some(v) = value {
                let value_ts = match v {
                    BoolIr::Const(b) => quote!(#b),
                    BoolIr::Dyn(arg) => self.arg_expr(arg, Usage::Consume),
                };
                let f = format_ident!("{field}");
                fills.push(quote! {
                    if #var.format.#f.is_none() {
                        #var.format.#f = ::core::option::Option::Some(#value_ts);
                    }
                });
            }
        }
        if let Some(font) = &style.font {
            fills.push(quote! {
                if #var.format.font.is_none() {
                    #var.format.font =
                        ::core::option::Option::Some(::std::borrow::Cow::Borrowed(#font));
                }
            });
        }
        if let Some(shadow) = style.shadow_color {
            fills.push(quote! {
                if #var.format.shadow_color.is_none() {
                    #var.format.shadow_color = ::core::option::Option::Some(#shadow);
                }
            });
        }
        if style.click.is_some() || style.hover.is_some() || style.insertion.is_some() {
            let events_only = Style {
                click: style.click.clone(),
                hover: style.hover.clone(),
                insertion: style.insertion.clone(),
                ..Style::default()
            };
            let interactivity = self.interactivity_tokens(&events_only);
            fills.push(quote! {
                let __tcm_ev: ::text_components::interactivity::Interactivity = #interactivity;
                if #var.interactions.insertion.is_none() {
                    #var.interactions.insertion = __tcm_ev.insertion;
                }
                if #var.interactions.click.is_none() {
                    #var.interactions.click = __tcm_ev.click;
                }
                if #var.interactions.hover.is_none() {
                    #var.interactions.hover = __tcm_ev.hover;
                }
            });
        }
        fills
    }

    pub(crate) fn hole_tokens(
        &mut self,
        arg: &HoleArg,
        kind: HoleKind,
        spec: Option<&str>,
        style: &Style,
    ) -> Ts {
        let var = format_ident!("__tcm_h");
        let make = match kind {
            HoleKind::ConstComponent => unreachable!("const holes never reach text!'s codegen"),
            HoleKind::Component => {
                let expr = self.arg_expr(arg, Usage::Consume);
                quote!(::core::convert::Into::<::text_components::TextComponent>::into(#expr))
            }
            HoleKind::Text => {
                let expr = self.arg_expr(arg, Usage::Borrow);
                let fmt = spec_format(spec);
                quote!(::text_components::TextComponent::plain(
                    ::std::format!(#fmt, #expr)
                ))
            }
        };
        let fills = self.fill_tokens(style, &var);
        quote! {
            {
                let mut #var: ::text_components::TextComponent = #make;
                #(#fills)*
                #var
            }
        }
    }

    pub(crate) fn runtime_leaf_tokens(&mut self, piece: &Piece) -> Ts {
        let style = piece.style();
        let mut base = style.clone();
        let mut assigns: Vec<Ts> = Vec::new();
        if let Some(ColorIr::Dyn(arg)) = &style.color {
            let e = self.dyn_color_tokens(arg);
            base.color = None;
            assigns.push(quote! {
                __tcm_c.format.color = ::core::option::Option::Some(#e);
            });
        }
        macro_rules! dyn_flag {
            ($f:ident) => {
                if let Some(BoolIr::Dyn(arg)) = &style.$f {
                    let e = self.arg_expr(arg, Usage::Consume);
                    base.$f = None;
                    let fid = format_ident!(stringify!($f));
                    assigns.push(quote! {
                        __tcm_c.format.#fid = ::core::option::Option::Some(#e);
                    });
                }
            };
        }
        dyn_flag!(bold);
        dyn_flag!(italic);
        dyn_flag!(underlined);
        dyn_flag!(strikethrough);
        dyn_flag!(obfuscated);
        let events_dyn = match &style.hover {
            Some(HoverIr::Text(pieces)) => grammar::pieces_have_dyn(pieces),
            Some(HoverIr::Dyn(_)) => true,
            None => false,
        } || match &style.click {
            Some(ClickIr::Action(_, segs)) => grammar::segs_have_dyn(segs),
            Some(ClickIr::Dyn(_)) => true,
            None => false,
        } || style
            .insertion
            .as_ref()
            .is_some_and(|segs| grammar::segs_have_dyn(segs));
        if events_dyn {
            base.hover = None;
            base.click = None;
            base.insertion = None;
            let events_only = Style {
                hover: style.hover.clone(),
                click: style.click.clone(),
                insertion: style.insertion.clone(),
                ..Style::default()
            };
            let interactivity = self.interactivity_tokens(&events_only);
            assigns.push(quote! {
                __tcm_c.interactions = #interactivity;
            });
        }
        let leaf = self.static_leaf_tokens(piece, &base);
        let id = self.fresh("P");
        self.statics.push(quote! {
            static #id: ::text_components::TextComponent = #leaf;
        });
        quote! {
            {
                let mut __tcm_c = ::core::clone::Clone::clone(&#id);
                #(#assigns)*
                __tcm_c
            }
        }
    }

    pub(crate) fn lang_tokens(&mut self, piece: &Piece) -> Ts {
        let Piece::Lang {
            key, args, style, ..
        } = piece
        else {
            unreachable!("lang_tokens only takes lang pieces");
        };
        let arity = args.len();
        let key_expr = match key {
            LangKey::Lit(k) => quote!(::std::borrow::Cow::Borrowed(#k)),
            LangKey::Dyn(arg) => {
                let e = self.arg_expr(arg, Usage::Borrow);
                quote!(::std::borrow::Cow::Borrowed(
                    ::core::borrow::Borrow::<
                        ::text_components::translation::Translation<#arity>,
                    >::borrow(#e)
                    .0
                ))
            }
        };
        let args_expr = if args.is_empty() {
            quote!(::text_components::Args::None)
        } else {
            let elems: Vec<Ts> = args.iter().map(|a| self.component_expr(a)).collect();
            quote!(::text_components::Args::Owned(
                ::std::boxed::Box::<[::text_components::TextComponent]>::from([#(#elems),*])
            ))
        };
        let var = format_ident!("__tcm_h");
        let fills = self.fill_tokens(style, &var);
        quote! {
            {
                let mut #var: ::text_components::TextComponent =
                    ::text_components::TextComponent::translated(
                        ::text_components::translation::TranslatedMessage {
                            key: #key_expr,
                            fallback: ::core::option::Option::None,
                            args: #args_expr,
                        }
                    );
                #(#fills)*
                #var
            }
        }
    }

    pub(crate) fn component_expr(&mut self, pieces: &[Piece]) -> Ts {
        if !grammar::pieces_have_dyn(pieces) {
            return self.static_component_tokens(pieces);
        }
        if let [
            Piece::Hole {
                arg,
                kind,
                spec,
                style,
                ..
            },
        ] = pieces
        {
            return self.hole_tokens(arg, *kind, spec.as_deref(), style);
        }
        if let [piece @ Piece::Lang { .. }] = pieces {
            return self.lang_tokens(piece);
        }
        if let [piece @ (Piece::Text { .. } | Piece::Keybind { .. })] = pieces {
            return self.runtime_leaf_tokens(piece);
        }
        let n = pieces.len();
        let pushes: Vec<Ts> = pieces
            .iter()
            .map(|piece| match piece {
                Piece::Hole {
                    arg,
                    kind,
                    spec,
                    style,
                    ..
                } => {
                    let hole = self.hole_tokens(arg, *kind, spec.as_deref(), style);
                    quote!(__tcm_children.push(#hole);)
                }
                piece @ Piece::Lang { .. } if piece.has_dyn() => {
                    let leaf = self.lang_tokens(piece);
                    quote!(__tcm_children.push(#leaf);)
                }
                piece if piece.style().has_dyn() => {
                    let leaf = self.runtime_leaf_tokens(piece);
                    quote!(__tcm_children.push(#leaf);)
                }
                piece => {
                    let leaf = self.static_leaf_tokens(piece, piece.style());
                    let id = self.fresh("P");
                    self.statics.push(quote! {
                        static #id: ::text_components::TextComponent = #leaf;
                    });
                    quote!(__tcm_children.push(::core::clone::Clone::clone(&#id));)
                }
            })
            .collect();
        let root = root_tokens(quote!(::std::borrow::Cow::Owned(__tcm_children)));
        quote! {
            {
                let mut __tcm_children: ::std::vec::Vec<::text_components::TextComponent> =
                    ::std::vec::Vec::with_capacity(#n);
                #(#pushes)*
                #root
            }
        }
    }

    pub(crate) fn static_component_tokens(&mut self, pieces: &[Piece]) -> Ts {
        let leaves: Vec<Ts> = pieces
            .iter()
            .map(|p| self.static_leaf_tokens(p, p.style()))
            .collect();
        if leaves.len() == 1 {
            return leaves
                .into_iter()
                .next()
                .expect("length was just checked to be one");
        }
        let n = leaves.len();
        let parts_id = self.fresh("PARTS");
        self.statics.push(quote! {
            static #parts_id: [::text_components::TextComponent; #n] = [#(#leaves),*];
        });
        root_tokens(quote!(::std::borrow::Cow::Borrowed(&#parts_id)))
    }
}

// `..expr` struct update in const position drops overwritten fields, which const-eval rejects
pub(crate) fn root_tokens(children: Ts) -> Ts {
    quote!(::text_components::TextComponent {
        content: ::text_components::content::Content::Text {
            text: ::std::borrow::Cow::Borrowed(""),
        },
        children: #children,
        format: ::text_components::format::Format {
            color: ::core::option::Option::None,
            font: ::core::option::Option::None,
            bold: ::core::option::Option::None,
            italic: ::core::option::Option::None,
            underlined: ::core::option::Option::None,
            strikethrough: ::core::option::Option::None,
            obfuscated: ::core::option::Option::None,
            shadow_color: ::core::option::Option::None,
        },
        interactions: ::text_components::interactivity::Interactivity {
            insertion: ::core::option::Option::None,
            click: ::core::option::Option::None,
            hover: ::core::option::Option::None,
        },
    })
}
