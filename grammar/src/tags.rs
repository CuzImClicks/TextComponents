/// (alias, canonical, rgb) for every vanilla named color.
pub const NAMED_COLORS: &[(&str, &str, (u8, u8, u8))] = &[
    ("black", "black", (0x00, 0x00, 0x00)),
    ("dark_blue", "dark_blue", (0x00, 0x00, 0xAA)),
    ("dark_green", "dark_green", (0x00, 0xAA, 0x00)),
    ("dark_aqua", "dark_aqua", (0x00, 0xAA, 0xAA)),
    ("dark_red", "dark_red", (0xAA, 0x00, 0x00)),
    ("dark_purple", "dark_purple", (0xAA, 0x00, 0xAA)),
    ("gold", "gold", (0xFF, 0xAA, 0x00)),
    ("gray", "gray", (0xAA, 0xAA, 0xAA)),
    ("grey", "gray", (0xAA, 0xAA, 0xAA)),
    ("dark_gray", "dark_gray", (0x55, 0x55, 0x55)),
    ("dark_grey", "dark_gray", (0x55, 0x55, 0x55)),
    ("blue", "blue", (0x55, 0x55, 0xFF)),
    ("green", "green", (0x55, 0xFF, 0x55)),
    ("aqua", "aqua", (0x55, 0xFF, 0xFF)),
    ("red", "red", (0xFF, 0x55, 0x55)),
    ("light_purple", "light_purple", (0xFF, 0x55, 0xFF)),
    ("yellow", "yellow", (0xFF, 0xFF, 0x55)),
    ("white", "white", (0xFF, 0xFF, 0xFF)),
];

pub(crate) fn all_tag_names() -> impl Iterator<Item = &'static str> {
    NAMED_COLORS.iter().map(|(alias, ..)| *alias).chain([
        "b",
        "bold",
        "i",
        "em",
        "italic",
        "u",
        "underlined",
        "st",
        "strikethrough",
        "obf",
        "obfuscated",
        "reset",
        "newline",
        "br",
        "hover",
        "click",
        "font",
        "insert",
        "insertion",
        "shadow",
        "shadow_color",
        "key",
        "keybind",
        "lang",
        "tr",
        "translate",
        "lang_or",
        "tr_or",
        "translate_or",
        "color",
        "colour",
        "c",
        "gradient",
        "rainbow",
        "score",
        "selector",
        "sel",
        "nbt",
        "data",
        "sprite",
        "head",
        "transition",
    ])
}

pub(crate) const SUPPORTED_SUMMARY: &str = "supported tags: named colors (<red>, <dark_gray>, …), <#RRGGBB>, \
     <b>/<i>/<u>/<st>/<obf>, <reset>, <newline>, <font:…>, <shadow:red:0.5>, \
     <key:key.jump>, <lang:key:'arg'>, <lang_or:key:'fallback'>, <gradient:#a:#b>, \
     <rainbow>, <transition:red:blue:0.5>, <insert:'…'>, <hover:show_text:'…'>, \
     <hover:show_item:'minecraft:stone':3>, <hover:show_entity:pig:uuid>, \
     <click:run_command:'…'> (and open_url, suggest_command, copy_to_clipboard, \
     change_page, show_dialog), <score:name:objective>, <selector:@a>, \
     <nbt:entity:'@s':Health>, <sprite:item/emerald>, <head:Notch>";

pub(crate) const COLOR_HELP: &str = "for example <color:red> or <color:#ff0000>";

pub(crate) fn canonical_tag(name: &str) -> String {
    match name {
        "b" => "bold".into(),
        "i" | "em" => "italic".into(),
        "u" => "underlined".into(),
        "st" => "strikethrough".into(),
        "obf" => "obfuscated".into(),
        "shadow_color" => "shadow".into(),
        "keybind" => "key".into(),
        "insertion" => "insert".into(),
        "c" | "colour" => "color".into(),
        "tr" | "translate" => "lang".into(),
        "tr_or" | "translate_or" => "lang_or".into(),
        "sel" => "selector".into(),
        "data" => "nbt".into(),
        n if n.starts_with('#') || n.starts_with('{') => "color".into(),
        n => NAMED_COLORS
            .iter()
            .find(|(alias, ..)| *alias == n)
            .map_or_else(|| n.to_string(), |(_, canon, _)| (*canon).to_string()),
    }
}
