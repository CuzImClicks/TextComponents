//! Encoding a component to network NBT and reading it back.
use simdnbt::owned::{BaseNbt, Nbt, NbtCompound, NbtTag};
use text_components::{
    Modifier, Style, TextComponent,
    format::Color,
    interactivity::{ClickEvent, HoverEvent},
    nbt::{NbtBuilder, ToSNBT},
    resolving::NoResolutor,
};

fn main() -> Result<(), String> {
    let nbt = Nbt::Some(BaseNbt::new(
        "",
        NbtCompound::from_values(vec![
            ("double".into(), NbtTag::Double(12.)),
            (
                "byteArray".into(),
                NbtTag::ByteArray(vec![1, 4, 7, 3, 74, 4, 65]),
            ),
            ("string".into(), NbtTag::String("This is a text".into())),
        ]),
    ));
    let component = TextComponent::nbt_display(nbt);
    println!(
        "tellraw @p {}",
        component.build(&NoResolutor, NbtBuilder).to_snbt()
    );
    println!("{component:p}");

    let nbt = "Holly molly I can get TextComponents from NBTs!"
        .color(Color::Red)
        .add_children(vec![
            "\n This has a Hover Event!"
                .hover_event(HoverEvent::show_text("This is a hover event"))
                .color(Color::Gold),
            "\n This has a ClickEvent!".click_event(ClickEvent::suggest_command(
                "/tell \"Guys, I'm very happy!\"",
            )),
        ])
        .build(&NoResolutor, NbtBuilder);
    println!("{nbt:?}");
    let component =
        TextComponent::from_nbt(&nbt).ok_or(String::from("Cannot recompose the TextComponent!"))?;
    println!("{component:?}");
    println!("{component:p}");
    Ok(())
}
