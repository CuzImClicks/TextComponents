use crate::{TextComponent, format::Color};
use simdnbt::Mutf8Str;

use super::{TAG_COMPOUND, TAG_LIST, TAG_STRING};

/// Java Modified UTF-8 payload, no length prefix.
#[doc(hidden)]
pub fn write_mutf8_body(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(Mutf8Str::from_str(s).as_bytes());
}

/// u16 BE length + Java Modified UTF-8 payload.
#[doc(hidden)]
pub fn write_mutf8(buf: &mut Vec<u8>, s: &str) {
    let at = buf.len();
    buf.extend_from_slice(&[0, 0]);
    write_mutf8_body(buf, s);
    let n = nbt_len(buf.len() - at - 2);
    buf[at..at + 2].copy_from_slice(&n.to_be_bytes());
}

/// The u16 length prefix of an NBT string.
///
/// # Panics
/// If `len` is over 65535 bytes.
#[doc(hidden)]
#[track_caller]
#[must_use]
pub fn nbt_len(len: usize) -> u16 {
    #[cold]
    #[inline(never)]
    #[track_caller]
    fn too_long(len: usize) -> ! {
        panic!("NBT string is {len} bytes, over the 65535-byte limit")
    }
    match u16::try_from(len) {
        Ok(n) => n,
        Err(_) => too_long(len),
    }
}

/// u16 BE length + the color's codec name ("aqua", "#RRGGBB").
#[doc(hidden)]
pub fn write_color(buf: &mut Vec<u8>, color: &Color) {
    let name = color.codec_name();
    buf.extend_from_slice(&(name.len() as u16).to_be_bytes());
    buf.extend_from_slice(name.as_bytes());
}

/// A runtime component's own encoding, verbatim ([type][payload]).
#[doc(hidden)]
pub fn write_component_root(buf: &mut Vec<u8>, component: &TextComponent) {
    let tag = component.to_codec_nbt();
    let mut tmp = Vec::with_capacity(128);
    tag.write(&mut tmp);
    buf.extend_from_slice(&tmp);
}

/// A runtime component as the value of a compound entry.
#[doc(hidden)]
pub fn write_component_entry(buf: &mut Vec<u8>, name: &str, component: &TextComponent) {
    let tag = component.to_codec_nbt();
    let mut tmp = Vec::with_capacity(128);
    tag.write(&mut tmp);
    buf.push(tmp[0]);
    write_mutf8(buf, name);
    buf.extend_from_slice(&tmp[1..]);
}

/// A runtime component as a compound list element.
#[doc(hidden)]
pub fn write_component_elem(buf: &mut Vec<u8>, component: &TextComponent) {
    let tag = component.to_codec_nbt();
    let mut tmp = Vec::with_capacity(128);
    tag.write(&mut tmp);
    if tmp.first() == Some(&TAG_COMPOUND) {
        buf.extend_from_slice(&tmp[1..]);
    } else {
        buf.extend_from_slice(&[TAG_STRING, 0, 0]);
        buf.extend_from_slice(&tmp[1..]);
        buf.push(0);
    }
}

/// An already-encoded component as the whole message: its bytes, verbatim.
#[doc(hidden)]
pub fn write_encoded_root(buf: &mut Vec<u8>, encoded: &[u8]) {
    buf.extend_from_slice(encoded);
}

/// An already-encoded component as a compound-list element.
#[doc(hidden)]
pub fn write_encoded_elem(buf: &mut Vec<u8>, encoded: &[u8]) {
    let Some((tag, payload)) = encoded.split_first() else {
        return;
    };
    if *tag == TAG_COMPOUND {
        buf.extend_from_slice(payload);
    } else {
        buf.extend_from_slice(&[TAG_STRING, 0, 0]);
        buf.extend_from_slice(payload);
        buf.push(0);
    }
}

/// An already-encoded component as the value of a named entry.
#[doc(hidden)]
pub fn write_encoded_entry(buf: &mut Vec<u8>, name: &str, encoded: &[u8]) {
    let Some((tag, payload)) = encoded.split_first() else {
        return;
    };
    buf.push(*tag);
    write_mutf8(buf, name);
    buf.extend_from_slice(payload);
}

pub(super) fn write_wrapped(buf: &mut Vec<u8>, style: &TextComponent, encoded: &[u8]) {
    let mut tmp = Vec::with_capacity(64);
    style.to_codec_nbt().write(&mut tmp);
    if let Some((&TAG_COMPOUND, payload)) = tmp.split_first() {
        buf.extend_from_slice(&payload[..payload.len() - 1]);
    } else {
        write_encoded_elem(buf, encoded);
        return;
    }
    buf.push(TAG_LIST);
    write_mutf8(buf, "extra");
    buf.push(TAG_COMPOUND);
    buf.extend_from_slice(&1i32.to_be_bytes());
    write_encoded_elem(buf, encoded);
    buf.push(0);
}
