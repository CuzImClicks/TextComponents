use std::{convert::Infallible, sync::Arc};

#[cfg(feature = "custom")]
use crate::custom::CustomData;
use crate::{
    TextComponent,
    content::{Content, Object, Resolvable},
    interactivity::{HoverEvent, MaybeStatic},
};

/// Fills in the content this crate cannot render on its own.
pub trait TextResolutor {
    /// Resolves content that needs no lookup, by default a clone.
    fn resolve_other(&self, content: &Content) -> TextComponent {
        TextComponent::from(content.clone())
    }
    /// Resolves a scoreboard, entity, or NBT value.
    fn resolve_content(&self, resolvable: &Resolvable) -> TextComponent;
    /// Resolves custom content, [None] to drop it.
    #[cfg(feature = "custom")]
    fn resolve_custom(&self, data: &CustomData) -> Option<TextComponent>;
    /// The translation for a key, [None] when it is unknown.
    fn translate(&self, key: &str) -> Option<String>;
    /// Splits a translated string into literal parts, each paired with the 1-based argument that follows it, or 0 for none.
    fn split_translation(&self, text: String) -> Vec<(String, usize)> {
        let mut positions = vec![(0, 0, 0), (text.len(), 0, 0)];
        for i in 1..=8 {
            for (pos, _) in text.match_indices(&format!("%{i}$s")) {
                positions.push((pos, i, 4usize));
            }
        }
        for (counter, (pos, _)) in (1..).zip(text.match_indices("%s")) {
            positions.push((pos, counter, 2usize));
        }
        positions.sort_by_key(|(pos, _, _)| *pos);
        let mut translation = vec![];
        let mut positions = positions.into_iter().peekable();
        while let Some((pos, _, size)) = positions.next() {
            let Some(next) = positions.peek() else {
                break;
            };
            translation.push((text[pos + size..next.0].to_string(), next.1));
        }
        translation
    }
}

/// A resolver for component contents that may fail.
pub trait TryTextResolutor {
    type Error;

    /// Resolves content that needs no lookup, by default a clone.
    fn try_resolve_other(&self, content: &Content) -> Result<TextComponent, Self::Error> {
        Ok(TextComponent::from(content.clone()))
    }

    /// Resolves a scoreboard, entity, or NBT value; `recursion_depth` is how deep this content sits below the root component, and anything past 100 is left unresolved.
    fn try_resolve_content(
        &self,
        resolvable: &Resolvable,
        recursion_depth: usize,
    ) -> Result<TextComponent, Self::Error>;

    /// Resolves custom content, [None] to drop it.
    #[cfg(feature = "custom")]
    fn try_resolve_custom(&self, data: &CustomData) -> Result<Option<TextComponent>, Self::Error> {
        let _ = data;
        Ok(None)
    }
}

impl<T: TextResolutor> TextResolutor for Arc<T> {
    fn resolve_content(&self, resolvable: &Resolvable) -> TextComponent {
        (**self).resolve_content(resolvable)
    }

    #[cfg(feature = "custom")]
    fn resolve_custom(&self, data: &CustomData) -> Option<TextComponent> {
        (**self).resolve_custom(data)
    }

    fn translate(&self, key: &str) -> Option<String> {
        (**self).translate(key)
    }

    fn split_translation(&self, text: String) -> Vec<(String, usize)> {
        (**self).split_translation(text)
    }
}

impl<T: TryTextResolutor> TryTextResolutor for Arc<T> {
    type Error = T::Error;

    fn try_resolve_other(&self, content: &Content) -> Result<TextComponent, Self::Error> {
        (**self).try_resolve_other(content)
    }

    fn try_resolve_content(
        &self,
        resolvable: &Resolvable,
        recursion_depth: usize,
    ) -> Result<TextComponent, Self::Error> {
        (**self).try_resolve_content(resolvable, recursion_depth)
    }

    #[cfg(feature = "custom")]
    fn try_resolve_custom(&self, data: &CustomData) -> Result<Option<TextComponent>, Self::Error> {
        (**self).try_resolve_custom(data)
    }
}

/// A resolver that renders every lookup as a placeholder.
pub struct NoResolutor;
impl TextResolutor for NoResolutor {
    fn resolve_content(&self, resolvable: &Resolvable) -> TextComponent {
        match resolvable {
            Resolvable::Scoreboard { objective, .. } => {
                TextComponent::plain(format!("[Score: {objective}]"))
            }
            Resolvable::Entity { selector, .. } => {
                TextComponent::plain(format!("[Entity: {selector}]"))
            }
            Resolvable::NBT { path, .. } => TextComponent::plain(format!("[Nbt: {path}]")),
        }
    }

    #[cfg(feature = "custom")]
    fn resolve_custom(&self, data: &CustomData) -> Option<TextComponent> {
        Some(TextComponent::plain(data.id.clone()))
    }

    fn translate(&self, _key: &str) -> Option<String> {
        None
    }
}

impl TextComponent {
    /// Resolves this component, then renders it with `target`.
    pub fn build<R: TextResolutor + ?Sized, S: BuildTarget>(
        &self,
        resolutor: &R,
        target: S,
    ) -> S::Result {
        target.build_component(resolutor, &self.resolve(resolutor))
    }

    /// Fills in this component and everything below it.
    #[must_use]
    pub fn resolve<R: TextResolutor + ?Sized>(&self, resolutor: &R) -> TextComponent {
        match self.try_resolve(&InfallibleResolver(resolutor)) {
            Ok(component) => component,
            Err(error) => match error {},
        }
    }

    /// Fills in this component and everything below it, stopping at the first error.
    pub fn try_resolve<R: TryTextResolutor + ?Sized>(
        &self,
        resolutor: &R,
    ) -> Result<TextComponent, R::Error> {
        self.try_resolve_at_depth(resolutor, 0)
    }

    /// Resolves content nested inside a resolver-provided value.
    pub fn try_resolve_from_depth<R: TryTextResolutor + ?Sized>(
        &self,
        resolutor: &R,
        recursion_depth: usize,
    ) -> Result<TextComponent, R::Error> {
        self.try_resolve_at_depth(resolutor, recursion_depth)
    }

    fn try_resolve_at_depth<R: TryTextResolutor + ?Sized>(
        &self,
        resolutor: &R,
        recursion_depth: usize,
    ) -> Result<TextComponent, R::Error> {
        if recursion_depth > 100 {
            return Ok(self.clone());
        }

        let content_depth = recursion_depth + 1;
        let mut component = match &self.content {
            Content::Translate(message) => {
                let mut message = message.clone();
                if !message.args.is_none() {
                    let args = message
                        .args
                        .iter()
                        .map(|arg| arg.try_resolve_at_depth(resolutor, content_depth))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice();
                    message.args = crate::Args::Owned(args);
                }
                resolutor.try_resolve_other(&Content::Translate(message))?
            }
            Content::Object(object) => {
                let mut object = object.clone();
                let fallback = match &mut object {
                    Object::Atlas { fallback, .. } | Object::Player { fallback, .. } => fallback,
                };
                if let Some(value) = fallback {
                    let resolved = value.try_resolve_at_depth(resolutor, content_depth)?;
                    *value = MaybeStatic::Owned(Box::new(resolved));
                }
                resolutor.try_resolve_other(&Content::Object(object))?
            }
            Content::Resolvable(resolvable) => {
                let mut resolvable = resolvable.clone();
                match &mut resolvable {
                    Resolvable::Entity { separator, .. } | Resolvable::NBT { separator, .. } => {
                        if let Some(value) = separator {
                            **value = value.try_resolve_at_depth(resolutor, content_depth)?;
                        }
                    }
                    Resolvable::Scoreboard { .. } => {}
                }
                resolutor.try_resolve_content(&resolvable, content_depth)?
            }
            #[cfg(feature = "custom")]
            Content::Custom(data) => resolutor.try_resolve_custom(data)?.unwrap_or_default(),
            content => resolutor.try_resolve_other(content)?,
        };

        component.children.to_mut().append(
            &mut self
                .children
                .iter()
                .map(|child| child.try_resolve_at_depth(resolutor, recursion_depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let mut interactions = self.interactions.clone();
        if let Some(hover) = &interactions.hover
            && let HoverEvent::ShowText { value } = hover.get()
        {
            let resolved = value.try_resolve_at_depth(resolutor, recursion_depth + 1)?;
            interactions.hover = Some(MaybeStatic::Owned(Box::new(HoverEvent::ShowText {
                value: MaybeStatic::Owned(Box::new(resolved)),
            })));
        }
        interactions.mix(&mut component.interactions);
        component.format = self.format.mix(&component.format);

        Ok(component)
    }
}

struct InfallibleResolver<'a, R: ?Sized>(&'a R);

impl<R: TextResolutor + ?Sized> TryTextResolutor for InfallibleResolver<'_, R> {
    type Error = Infallible;

    fn try_resolve_other(&self, content: &Content) -> Result<TextComponent, Self::Error> {
        Ok(self.0.resolve_other(content))
    }

    fn try_resolve_content(
        &self,
        resolvable: &Resolvable,
        _recursion_depth: usize,
    ) -> Result<TextComponent, Self::Error> {
        Ok(self.0.resolve_content(resolvable))
    }

    #[cfg(feature = "custom")]
    fn try_resolve_custom(&self, data: &CustomData) -> Result<Option<TextComponent>, Self::Error> {
        Ok(self.0.resolve_custom(data))
    }
}

/// Renders a resolved component into some output form.
pub trait BuildTarget {
    type Result;
    /// Renders `component` together with its children.
    fn build_component<R: TextResolutor + ?Sized>(
        &self,
        resolutor: &R,
        component: &TextComponent,
    ) -> Self::Result;
}
