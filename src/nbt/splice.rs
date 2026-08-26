use crate::TextComponent;
use std::fmt;

use super::writers::{
    write_component_elem, write_component_entry, write_component_root, write_encoded_elem,
    write_encoded_entry, write_encoded_root, write_mutf8, write_mutf8_body, write_wrapped,
};
use super::{TAG_COMPOUND, TAG_STRING};

/// Writes the value of a `{@}` hole into a `text_nbt!` buffer.
pub trait SpliceComponent {
    /// The whole message.
    fn splice_root(self, buf: &mut Vec<u8>, style: Option<&TextComponent>);
    /// One element of a compound list (`extra`, `with`).
    fn splice_elem(self, buf: &mut Vec<u8>, style: Option<&TextComponent>);
    /// The value of a named compound entry, such as a hover's `value`.
    fn splice_entry(self, buf: &mut Vec<u8>, name: &str, style: Option<&TextComponent>);
}

impl<T: Into<TextComponent>> SpliceComponent for T {
    fn splice_root(self, buf: &mut Vec<u8>, style: Option<&TextComponent>) {
        write_component_root(buf, &inherit(self.into(), style));
    }
    fn splice_elem(self, buf: &mut Vec<u8>, style: Option<&TextComponent>) {
        write_component_elem(buf, &inherit(self.into(), style));
    }
    fn splice_entry(self, buf: &mut Vec<u8>, name: &str, style: Option<&TextComponent>) {
        write_component_entry(buf, name, &inherit(self.into(), style));
    }
}

fn inherit(mut component: TextComponent, style: Option<&TextComponent>) -> TextComponent {
    let Some(style) = style else {
        return component;
    };
    component.format = component.format.mix(&style.format);
    if component.interactions.insertion.is_none() {
        component
            .interactions
            .insertion
            .clone_from(&style.interactions.insertion);
    }
    if component.interactions.click.is_none() {
        component
            .interactions
            .click
            .clone_from(&style.interactions.click);
    }
    if component.interactions.hover.is_none() {
        component
            .interactions
            .hover
            .clone_from(&style.interactions.hover);
    }
    component
}

impl crate::EncodedComponent {
    /// Splices these bytes in as the whole message.
    pub fn splice_root(self, buf: &mut Vec<u8>, style: Option<&TextComponent>) {
        match style {
            None => write_encoded_root(buf, self.as_bytes()),
            Some(style) => {
                buf.push(TAG_COMPOUND);
                write_wrapped(buf, style, self.as_bytes());
            }
        }
    }

    /// Splices these bytes in as one element of a compound list.
    pub fn splice_elem(self, buf: &mut Vec<u8>, style: Option<&TextComponent>) {
        match style {
            None => write_encoded_elem(buf, self.as_bytes()),
            Some(style) => write_wrapped(buf, style, self.as_bytes()),
        }
    }

    /// Splices these bytes in as the value of a named compound entry.
    pub fn splice_entry(self, buf: &mut Vec<u8>, name: &str, style: Option<&TextComponent>) {
        match style {
            None => write_encoded_entry(buf, name, self.as_bytes()),
            Some(style) => {
                buf.push(TAG_COMPOUND);
                write_mutf8(buf, name);
                write_wrapped(buf, style, self.as_bytes());
            }
        }
    }

    /// Carries the `{@const}` hint a `text_nbt!` hole fires.
    #[doc(hidden)]
    #[deprecated(note = "this component is already encoded; if it is a `const`, write \
                {@const NAME} so its bytes are copied in at compile time instead of on every call")]
    pub const fn already_encoded(&self) {}
}

/// One piece of a compile-time byte run.
#[doc(hidden)]
pub enum SplicePart<'a> {
    Raw(&'a [u8]),
    Root(&'a [u8]),
    Elem(&'a [u8]),
    /// The name goes in as it stands, so it has to be ASCII.
    Entry(&'a str, &'a [u8]),
    /// The value of an NBT string field: u16 BE length + MUTF-8 payload.
    Str(&'a str),
}

/// The size of the array [`splice_bytes`] fills for these parts.
#[doc(hidden)]
#[must_use]
pub const fn splice_len(parts: &[SplicePart]) -> usize {
    let mut len = 0;
    let mut i = 0;
    while i < parts.len() {
        len += match &parts[i] {
            SplicePart::Raw(bytes) | SplicePart::Root(bytes) => bytes.len(),
            SplicePart::Elem(bytes) => match splice_tag(bytes) {
                TAG_COMPOUND => bytes.len() - 1,
                _ => bytes.len() + 3,
            },
            SplicePart::Entry(name, bytes) => {
                splice_tag(bytes);
                bytes.len() + 2 + name.len()
            }
            SplicePart::Str(text) => 2 + mutf8_len(text),
        };
        i += 1;
    }
    len
}

/// The run itself: literal bytes with every `{@const}` splice folded in.
#[doc(hidden)]
#[must_use]
pub const fn splice_bytes<const N: usize>(parts: &[SplicePart]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut at = 0;
    let mut i = 0;
    while i < parts.len() {
        at = match &parts[i] {
            SplicePart::Raw(bytes) | SplicePart::Root(bytes) => copy(&mut out, at, bytes, 0),
            SplicePart::Elem(bytes) => {
                if splice_tag(bytes) == TAG_COMPOUND {
                    copy(&mut out, at, bytes, 1)
                } else {
                    out[at] = TAG_STRING;
                    let end = copy(&mut out, at + 3, bytes, 1);
                    out[end] = 0;
                    end + 1
                }
            }
            SplicePart::Entry(name, bytes) => {
                out[at] = splice_tag(bytes);
                let named = write_name(&mut out, at + 1, name);
                copy(&mut out, named, bytes, 1)
            }
            SplicePart::Str(text) => write_mutf8_const(&mut out, at, text),
        };
        i += 1;
    }
    assert!(
        at == N,
        "spliced run does not fill the array splice_len sized for it"
    );
    out
}

/// How many bytes the MUTF-8 of a string takes, without the length prefix.
const fn mutf8_len(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut len = 0;
    let mut i = 0;
    while i < bytes.len() {
        len += mutf8_size(bytes[i]);
        i += utf8_step(bytes[i]);
    }
    assert!(
        len <= u16::MAX as usize,
        "const text is over the 65535 bytes an NBT string can carry"
    );
    len
}

/// The length of the UTF-8 sequence a leading byte opens.
const fn utf8_step(lead: u8) -> usize {
    if lead < 0x80 {
        1
    } else if lead < 0xE0 {
        2
    } else if lead < 0xF0 {
        3
    } else {
        4
    }
}

/// What that sequence becomes in MUTF-8: NUL grows, supplementary code points become a pair.
const fn mutf8_size(lead: u8) -> usize {
    if lead == 0 {
        2
    } else if lead >= 0xF0 {
        6
    } else {
        utf8_step(lead)
    }
}

/// Mirrors `write_mutf8`: u16 BE length + MUTF-8 payload.
const fn write_mutf8_const(out: &mut [u8], at: usize, text: &str) -> usize {
    let len = mutf8_len(text);
    out[at] = (len >> 8) as u8;
    out[at + 1] = len as u8;
    let bytes = text.as_bytes();
    let mut at = at + 2;
    let mut i = 0;
    while i < bytes.len() {
        let lead = bytes[i];
        if lead == 0 {
            out[at] = 0xC0;
            out[at + 1] = 0x80;
            at += 2;
            i += 1;
        } else if lead < 0xF0 {
            let n = utf8_step(lead);
            let mut k = 0;
            while k < n {
                out[at + k] = bytes[i + k];
                k += 1;
            }
            at += n;
            i += n;
        } else {
            let c = (((lead & 0x07) as u32) << 18)
                | (((bytes[i + 1] & 0x3F) as u32) << 12)
                | (((bytes[i + 2] & 0x3F) as u32) << 6)
                | ((bytes[i + 3] & 0x3F) as u32);
            let v = c - 0x1_0000;
            at = push3(out, at, 0xD800 + (v >> 10));
            at = push3(out, at, 0xDC00 + (v & 0x3FF));
            i += 4;
        }
    }
    at
}

const fn push3(out: &mut [u8], at: usize, c: u32) -> usize {
    out[at] = 0xE0 | (c >> 12) as u8;
    out[at + 1] = 0x80 | ((c >> 6) & 0x3F) as u8;
    out[at + 2] = 0x80 | (c & 0x3F) as u8;
    at + 3
}

const fn splice_tag(bytes: &[u8]) -> u8 {
    assert!(
        !bytes.is_empty(),
        "a spliced component has no bytes — an encoding is [tag type][payload]"
    );
    match bytes[0] {
        TAG_STRING | TAG_COMPOUND => bytes[0],
        _ => panic!("a spliced component must be an NBT string or compound"),
    }
}

const fn copy(out: &mut [u8], at: usize, bytes: &[u8], from: usize) -> usize {
    let mut at = at;
    let mut i = from;
    while i < bytes.len() {
        out[at] = bytes[i];
        at += 1;
        i += 1;
    }
    at
}

const fn write_name(out: &mut [u8], at: usize, name: &str) -> usize {
    let bytes = name.as_bytes();
    assert!(
        bytes.len() <= u16::MAX as usize,
        "entry name over the 65535 bytes an NBT name can carry"
    );
    let mut i = 0;
    while i < bytes.len() {
        assert!(bytes[i].is_ascii(), "a spliced entry name has to be ASCII");
        i += 1;
    }
    out[at] = (bytes.len() >> 8) as u8;
    out[at + 1] = bytes.len() as u8;
    copy(out, at + 2, bytes, 0)
}

/// fmt::Write sink that MUTF-8-encodes each chunk straight into the buffer.
#[doc(hidden)]
pub struct Mutf8Fmt<'a>(pub &'a mut Vec<u8>);

impl fmt::Write for Mutf8Fmt<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_mutf8_body(self.0, s);
        Ok(())
    }
}
