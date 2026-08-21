//! Resolving server-side content, fallible and depth-limited.
use std::{cell::Cell, convert::Infallible};
use text_components::interactivity::MaybeStatic;

use text_components::{
    TextComponent,
    content::{Content, Object, Resolvable},
    interactivity::HoverEvent,
    resolving::TryTextResolutor,
};

struct Resolver {
    calls: Cell<usize>,
}

impl TryTextResolutor for Resolver {
    type Error = &'static str;

    fn try_resolve_content(
        &self,
        resolvable: &Resolvable,
        _recursion_depth: usize,
    ) -> Result<TextComponent, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        match resolvable {
            Resolvable::Entity { selector, .. } if selector == "@error" => Err("selector failed"),
            Resolvable::Entity { selector, .. } => {
                Ok(TextComponent::plain(format!("resolved:{selector}")))
            }
            _ => Ok(TextComponent::new()),
        }
    }
}

#[test]
fn fallible_resolution_propagates_content_errors() {
    let resolver = Resolver {
        calls: Cell::new(0),
    };
    let component = TextComponent::entity("@error", None);

    assert_eq!(component.try_resolve(&resolver), Err("selector failed"));
}

#[test]
fn resolution_reaches_hover_text_and_object_fallbacks() {
    let resolver = Resolver {
        calls: Cell::new(0),
    };
    let mut component = TextComponent::from(Object::Atlas {
        atlas: "minecraft:blocks".into(),
        sprite: "minecraft:item/diamond".into(),
        fallback: Some(Box::new(TextComponent::entity("@fallback", None))),
    });
    component.interactions.hover = Some(MaybeStatic::Owned(Box::new(HoverEvent::show_text(
        TextComponent::entity("@hover", None),
    ))));

    let output = component
        .try_resolve(&resolver)
        .expect("nested resolution should succeed");
    assert!(matches!(
        output.content,
        Content::Object(Object::Atlas {
            fallback: Some(ref fallback),
            ..
        }) if **fallback == TextComponent::plain("resolved:@fallback")
    ));
    assert!(matches!(
        output.interactions.hover.as_deref(),
        Some(HoverEvent::ShowText { value })
            if **value == TextComponent::plain("resolved:@hover")
    ));
    assert_eq!(resolver.calls.get(), 2);
}

#[test]
fn depth_limit_copies_remaining_components_without_resolving_them() {
    let resolver = Resolver {
        calls: Cell::new(0),
    };
    let unresolved = TextComponent::entity("@deep", None);
    let mut component = unresolved.clone();
    for _ in 0..=100 {
        let mut parent = TextComponent::new();
        parent.children.to_mut().push(component);
        component = parent;
    }

    let output = component
        .try_resolve(&resolver)
        .expect("depth limiting should not fail");
    let mut current = &output;
    for _ in 0..=100 {
        current = &current.children[0];
    }
    assert_eq!(current, &unresolved);
    assert_eq!(resolver.calls.get(), 0);
}

struct InfallibleResolver;

impl TryTextResolutor for InfallibleResolver {
    type Error = Infallible;

    fn try_resolve_content(
        &self,
        _resolvable: &Resolvable,
        _recursion_depth: usize,
    ) -> Result<TextComponent, Self::Error> {
        Ok(TextComponent::plain("resolved"))
    }
}

#[test]
fn try_resolve_supports_infallible_resolvers() {
    assert_eq!(
        TextComponent::entity("@s", None).try_resolve(&InfallibleResolver),
        Ok(TextComponent::plain("resolved"))
    );
}
