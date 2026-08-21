//! Parsing a component from SNBT.
use text_components::TextComponent;

fn main() {
    use std::io::{Write, stdin, stdout};
    let mut s = String::new();
    print!("/tellraw @s ");
    let _ = stdout().flush();
    stdin()
        .read_line(&mut s)
        .expect("Did not enter a correct string");
    if let Some('\n') = s.chars().next_back() {
        s.pop();
    }
    if let Some('\r') = s.chars().next_back() {
        s.pop();
    }
    let component = TextComponent::from_snbt(&s);
    match component {
        Ok(component) => {
            println!("{component:?}");
            println!("{component:p}");
        }
        Err(e) => eprintln!("{e}"),
    }
}
