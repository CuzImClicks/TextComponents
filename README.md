# Text Components RS

This is a library for easy implementation and usage of Minecraft's Text Components, designed for Java edition but extensible to match Bedrock's Components.

### Usage

You can make your first text component like this:

```rs
let component = TextComponent::plain("Hello World!");
```

Decorate it like this:

```rs
let component = component.color(Color::Red).bold(true);
```

Adding interactivility like this:

```rs
let component = component.insertion("Hello");

let component = component.hover_event(
    HoverEvent::show_text("Hello World!")
);

let component = component.click_event(
    ClickEvent::open_url("https://github.com/Steel-Foundation/TextComponents")
);
```

Once the component is ready to be sent or displayed only rests building it:

```rs
component.build(resolutor, PrettyTextBuilder);
// Equivalent of doing:
component.to_pretty(resolutor);
```

If you want to use serde you will need to do this instead:

```rs
component.resolve(resolutor).serialize(serializer);
```

### Displaying TextComponents

TextComponent implements Display for easy logging, as you can see, a component
needs to be resolved before building it into any format, by default it uses a static
reference to NoResolutor, but can be changed to a custom one with:\
(Resolutor must be static, or made inside the function call)

```rs
set_display_resolutor(&Resolutor);
```

A text component can be printed like a string like this:

```rs
println!("{}", component);
// With format (pretty):
println!("{:p}", component);
```

### Roadmap

- [x] Text Components
- [x] Build system
- [x] Resolution system
- [x] Parsing system
- [x] Translations build macro
- [x] Terminal integration
- [x] Serde integration
- [x] SimdNbt integration
- [x] MiniMessages integration
- [ ] Extensibility integration

### Test

To test the capabilities of the library you can execute:

```bash
cargo run --example main
```

With all the features:

```bash
cargo run --example main --features serde,nbt,custom
```
