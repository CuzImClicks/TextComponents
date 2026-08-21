use heck::ToShoutySnakeCase;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use serde_json::Value;
use std::fs;

fn count_parameters(text: &str) -> usize {
    let sequential = text.matches("%s").count();
    let mut positional = 0;
    for i in 1..=8 {
        if text.contains(&format!("%{i}$s")) {
            positional = positional.max(i);
        }
    }
    sequential.max(positional)
}

/// # Panics
/// If the language file at `path` cannot be read or parsed as JSON.
#[must_use]
pub fn build_translations(path: &str) -> TokenStream {
    println!("cargo:rerun-if-changed={path}");

    let lang_file =
        fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read {path} language file"));

    let translations: serde_json::Map<String, Value> =
        serde_json::from_str(&lang_file).unwrap_or_else(|_| panic!("Failed to parse {path}"));

    let mut stream = TokenStream::new();

    stream.extend(quote! {
        #![allow(dead_code)]
        use text_components::translation::Translation;
    });

    let mut translations_vec: Vec<_> = translations.iter().collect();
    translations_vec.sort_by_key(|(k, _)| *k);

    let mut used_names = rustc_hash::FxHashMap::default();

    for (key, value) in translations_vec {
        let Some(text) = value.as_str() else {
            eprintln!("Warning: Translation key '{key}' has non-string value, skipping");
            continue;
        };

        let param_count = count_parameters(text);

        if param_count > 8 {
            eprintln!(
                "Warning: Translation '{key}' has {param_count} parameters (max 8 supported), skipping"
            );
            continue;
        }

        let mut const_name_str = key.to_shouty_snake_case();

        if let Some(count) = used_names.get_mut(&const_name_str) {
            *count += 1;
            const_name_str = format!("{const_name_str}_{count}");
        } else {
            used_names.insert(const_name_str.clone(), 0);
        }

        let const_name = Ident::new(&const_name_str, Span::call_site());

        stream.extend(quote! {
            pub static #const_name: Translation<#param_count> = Translation(#key);
        });
    }

    stream
}
