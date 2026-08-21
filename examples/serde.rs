//! Serializing components and translations with serde.
#![expect(clippy::unwrap_used, reason = "example code")]
use text_components::{Args, Style, TextComponent, format::Color, translation::TranslatedMessage};

fn main() {
    let component: TextComponent = TranslatedMessage::new("key", Args::None)
        .color(Color::Blue)
        .bold(true);
    println!("{}", serde_json::to_string_pretty(&component).unwrap());
    let component: TextComponent = serde_json::from_str(
        "{
            \"text\": \"This is a Serde test\",
            \"color\": \"blue\",
            \"bold\": true
        }",
    )
    .unwrap();
    println!("{component:p}");
}
