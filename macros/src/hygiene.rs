use proc_macro2::{Group, Ident, Span, TokenStream as Ts, TokenTree};

pub(crate) fn is_internal(id: &Ident) -> bool {
    let name = id.to_string();
    name.starts_with("__tcm_") || name.starts_with("__TCM_")
}

pub(crate) fn hygienic(ts: Ts) -> Ts {
    ts.into_iter()
        .map(|tt| match tt {
            TokenTree::Ident(id) if is_internal(&id) => {
                let mut id = id;
                id.set_span(Span::mixed_site());
                TokenTree::Ident(id)
            }
            TokenTree::Group(group) => {
                let mut retagged = Group::new(group.delimiter(), hygienic(group.stream()));
                retagged.set_span(group.span());
                TokenTree::Group(retagged)
            }
            other => other,
        })
        .collect()
}
