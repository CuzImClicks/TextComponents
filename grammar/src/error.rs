#[cfg(feature = "render")]
use std::ops::Range;

#[cfg(feature = "render")]
use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};

/// A parse error at a char range inside the template.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Char range of the offending text in the template.
    pub range: (usize, usize),
    /// What went wrong.
    pub message: String,
    /// A suggested fix.
    pub help: Option<String>,
}

impl ParseError {
    pub(crate) fn new(range: (usize, usize), message: impl Into<String>) -> Self {
        Self {
            range,
            message: message.into(),
            help: None,
        }
    }
    pub(crate) fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Renders the message, the offending source line under a caret span, and the help line.
    #[cfg(feature = "render")]
    #[must_use]
    pub fn render(&self, src: &str) -> String {
        let mut group = Level::ERROR.no_name().primary_title(&self.message).element(
            Snippet::source(src).annotation(AnnotationKind::Primary.span(self.byte_span(src))),
        );
        if let Some(help) = &self.help {
            group = group.element(Level::HELP.message(help));
        }
        Renderer::plain().render(&[group])
    }

    /// [`Self::range`] is a char range; annotate-snippets wants byte offsets.
    #[cfg(feature = "render")]
    fn byte_span(&self, src: &str) -> Range<usize> {
        let bounds: Vec<usize> = src
            .char_indices()
            .map(|(i, _)| i)
            .chain([src.len()])
            .collect();
        let last = bounds.len() - 1;
        let start = self.range.0.min(last);
        // an empty span would underline nothing, so cover the next char
        let end = self.range.1.clamp(start, last).max((start + 1).min(last));
        bounds[start]..bounds[end]
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}

pub(crate) fn suggest<'a>(
    input: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    candidates
        .into_iter()
        .map(|c| (levenshtein(input, c), c))
        .filter(|(d, c)| *d <= (c.len().max(input.len()) / 3).max(1))
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}
