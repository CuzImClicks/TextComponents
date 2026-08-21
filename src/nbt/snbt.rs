use simdnbt::owned::{BaseNbt, Nbt, NbtCompound, NbtTag};
use std::ops::Deref as _;

/// Writes NBT back out as SNBT text.
pub trait ToSNBT {
    /// The value as SNBT.
    fn to_snbt(&self) -> String;
}

impl ToSNBT for Nbt {
    fn to_snbt(&self) -> String {
        match self {
            Nbt::Some(base) => base.to_snbt(),
            Nbt::None => String::new(),
        }
    }
}
impl ToSNBT for BaseNbt {
    fn to_snbt(&self) -> String {
        let mut child = String::new();
        if !self.name().is_empty() {
            if self.name().to_str().contains(':') {
                child = format!("\"{}\":", self.name());
            } else {
                child = format!("{}:", self.name());
            }
        }
        child.push_str(&self.deref().to_snbt());
        child
    }
}
impl ToSNBT for NbtCompound {
    fn to_snbt(&self) -> String {
        if self.len() == 1 {
            for (name, tag) in self.iter() {
                if name.is_empty() || name.to_str() == "text" {
                    return tag.to_snbt();
                }
            }
        }
        let mut snbt = vec![];
        for (name, tag) in self.iter() {
            let mut child = String::new();
            if !name.is_empty() {
                if name.to_str().contains(':') {
                    child = format!("\"{name}\":");
                } else {
                    child = format!("{name}:");
                }
            }
            child.push_str(&tag.to_snbt());
            snbt.push(child);
        }
        format!("{{{}}}", snbt.join(","))
    }
}
impl ToSNBT for NbtTag {
    fn to_snbt(&self) -> String {
        match self {
            NbtTag::Byte(n) => format!("{n}b"),
            NbtTag::Short(n) => format!("{n}s"),
            NbtTag::Int(n) => n.to_string(),
            NbtTag::Long(n) => format!("{n}l"),
            NbtTag::Float(n) => format!("{n:?}f"),
            NbtTag::Double(n) => format!("{n:?}d"),
            NbtTag::ByteArray(items) => format!(
                "[B;{}]",
                items
                    .iter()
                    .map(|n| format!("{n}b"))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            NbtTag::String(str) => format!(
                "\"{}\"",
                // TODO: Check escapable characters
                str.to_string()
                    .replace('\\', "\\\\")
                    .replace('\n', "\\n")
                    .replace('"', "\\\"")
                    .replace('\'', "\\'")
            ),
            NbtTag::List(items) => format!(
                "[{}]",
                items
                    .as_nbt_tags()
                    .iter()
                    .map(ToSNBT::to_snbt)
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            NbtTag::Compound(nbt) => nbt.to_snbt(),
            NbtTag::IntArray(items) => format!(
                "[I;{}]",
                items
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            NbtTag::LongArray(items) => format!(
                "[L;{}]",
                items
                    .iter()
                    .map(|n| format!("{n}l"))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
        }
    }
}
