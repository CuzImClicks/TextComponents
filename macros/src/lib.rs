//! `MiniMessage` templates checked at compile time. `text!` builds a
//! `TextComponent`, `text_nbt!` an `EncodedComponent` holding the message
//! encoded as network NBT bytes.

use proc_macro::TokenStream;

mod analysis;
mod codegen;
mod expand;
mod hygiene;
mod nbt;
mod parse;

/// A `TextComponent` from a `MiniMessage` template.
///
/// ```
/// # use text_components_macros::text;
/// # use text_components::TextComponent;
/// let name = "Notch";
/// let hearts = 9.5_f32;
/// let msg = text!("<red>{name}</red><gray> is on </gray><red>{hearts:.1} HP</red>");
///
/// const HEADER: TextComponent = text!("<yellow><b>Steel</b></yellow>");
/// ```
///
/// The first argument is a string literal template. Positional holes `{}` are
/// filled from the arguments after it, in order. Named holes `{name}` read the
/// variable `name` from the surrounding scope. Too many or too few arguments is
/// a compile error. A syntax error in the template is a compile error that
/// points at the place inside the string literal and carries a help line.
/// A template without holes can initialize a `const` or a `static`.
///
/// # Holes
///
/// - `{}`, `{name}`, `{name:.2}`, `{:>8}`: any `Display` value (just like format!)
/// - `{@}`, `{@name}`: anything `Into<TextComponent>`. The value is moved. If
///   the same name is used more than once, the earlier uses clone it.
/// - `<{color}>`, `<{}>`: a `Color` or a `&Color`. Close with `</color>`.
/// - `<b:{flag}>`, and the same for `i`, `u`, `st`, `obf`: a `bool`.
/// - `<hover:{event}>`: anything `Into<HoverEvent>`
/// - `<click:{event}>`: anything `Into<ClickEvent>`
/// - `<lang:{key}:'arg':'arg'>`: a `&Translation<N>`
///
/// # Tags
///
/// Tag names are case-insensitive; arguments and hole names keep their case.
///
/// - Colors: the vanilla names `<red>`, `<dark_gray>`, ... , `<#RRGGBB>`, `<color:red>`, `<color:#ff0000>`
/// - Formatting: `<bold>`, `<italic>`, `<underlined>`,
///   `<strikethrough>`, `<obfuscated>`
/// - `<font:minecraft:uniform>`
/// - `<shadow:#AARRGGBB>`, `<shadow:red:0.5>` also `#RRGGBB` or a color name. `<!shadow>` turns it off
/// - `<gradient:red:#f79459>` with two or more stops, and `<rainbow>`
/// - `<hover:show_text:'...'>`
/// - `<click:open_url:'...'>`, `<click:run_command:'...'>`,
///   `<click:suggest_command:'...'>`, `<click:copy_to_clipboard:'...'>`,
///   `<click:change_page:'3'>` (an integer, no holes),
///   `<click:show_dialog:'namespace:id'>`
/// - `<insert:'...'>`, `<insertion:'...'>`
/// - `<key:key.jump>`
/// - `<lang:key:'arg':'arg'>`
/// - `<lang_or:key:'fallback':'arg'>`, the fallback is shown when the client doesn't know that translation
/// - `<newline>`, `<br>` insert `\n`
/// - `<reset>` closes every open tag
/// - `<tag/>` closes itself right away
/// - Tag arguments are separated by `:`. An argument that contains `:` or `>`
///   must be quoted: `'...'` or `"..."`. Inside quotes, write `\'` (or `\"`)
///   for a literal quote of the kind that opened the argument.
/// - Escapes: `\<` for a literal `<`, `\\` for a backslash, `{{` and `}}` for
///   literal braces.
#[proc_macro]
pub fn text(input: TokenStream) -> TokenStream {
    expand::text(input)
}

/// An `EncodedComponent` from a `MiniMessage` template: the message encoded as
/// network NBT bytes, ready to send.
///
/// ```
/// # use text_components_macros::text_nbt;
/// # use text_components::EncodedComponent;
/// let tps = 19.87_f32;
/// let msg = text_nbt!("<gray>TPS: </gray><green>{tps:.1}</green>");
///
/// const HEADER: EncodedComponent = text_nbt!("\n<yellow>Steel Dev Build</yellow>\n");
/// ```
///
/// Same template syntax and argument rules as [`text!`]. What differs:
///
/// - Without holes, the bytes are computed at compile time and nothing is
///   allocated at run time.
/// - `{@}` and `{@name}` also accept an `EncodedComponent`. Its bytes are
///   copied in without decoding.
/// - `{@const NAME}`: `NAME` is a `const EncodedComponent` in scope, and its
///   bytes are copied into the template at compile time. Only `text_nbt!`
///   accepts this. Prefer it over `{@NAME}` for consts.
#[proc_macro]
pub fn text_nbt(input: TokenStream) -> TokenStream {
    expand::text_nbt(input)
}
