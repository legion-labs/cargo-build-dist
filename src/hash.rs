use semver::Version;
use sha2::{Digest, Sha256};
use std::{io::Write, path::PathBuf};

pub trait Hashable {
    fn hash(&self) -> String {
        let mut state = Sha256::new();
        self.as_hash_item().write_to(&mut state).unwrap();
        format!("sha256:{:x}", state.finalize())
    }

    fn as_hash_item(&self) -> HashItem<'_>;
}

/// A hash item is a composable part of of a hash and helps achieving
/// injectivity when hashing composite values.
#[derive(Debug, Clone)]
pub enum HashItem<'a> {
    String(&'a str),
    Version(&'a Version),
    Bytes(&'a [u8]),
    Path(&'a PathBuf),
    Named(&'a str, Box<HashItem<'a>>),
    List(Vec<HashItem<'a>>),
    Raw(Vec<u8>),
}

impl<'a> From<&'a str> for HashItem<'a> {
    fn from(val: &'a str) -> Self {
        HashItem::String(val)
    }
}

impl<'a> From<&'a String> for HashItem<'a> {
    fn from(val: &'a String) -> Self {
        HashItem::String(val)
    }
}

impl<'a> From<&'a Version> for HashItem<'a> {
    fn from(val: &'a Version) -> Self {
        HashItem::Version(val)
    }
}

impl<'a> From<&'a [u8]> for HashItem<'a> {
    fn from(val: &'a [u8]) -> Self {
        HashItem::Bytes(val)
    }
}

impl<'a> From<&'a Vec<u8>> for HashItem<'a> {
    fn from(val: &'a Vec<u8>) -> Self {
        HashItem::Bytes(val)
    }
}

impl<'a> From<&'a PathBuf> for HashItem<'a> {
    fn from(val: &'a PathBuf) -> Self {
        HashItem::Path(val)
    }
}

impl<'a> FromIterator<HashItem<'a>> for HashItem<'a> {
    fn from_iter<T: IntoIterator<Item = HashItem<'a>>>(iter: T) -> Self {
        Self::List(iter.into_iter().collect())
    }
}

impl<'a> HashItem<'a> {
    pub fn named(name: &'a str, item: impl Into<Self>) -> Self {
        HashItem::Named(name, Box::new(item.into()))
    }

    fn write_to(&self, w: &mut impl Write) -> std::io::Result<()> {
        match self {
            HashItem::String(s) => {
                w.write_all(b"s")?;
                w.write_all(&(s.len() as u128).to_be_bytes())?;
                w.write_all(s.as_bytes())?;
            }
            HashItem::Version(v) => {
                w.write_all(b"v")?;
                w.write_all(v.to_string().as_bytes())?;
            }
            HashItem::Bytes(b) => {
                w.write_all(b"b")?;
                w.write_all(&(b.len() as u128).to_be_bytes())?;
                w.write_all(b)?;
            }
            HashItem::Path(p) => {
                w.write_all(b"p")?;
                let s = p.to_string_lossy().into_owned();
                w.write_all(&(s.len() as u128).to_be_bytes())?;
                w.write_all(s.as_bytes())?;
            }
            HashItem::Named(name, item) => {
                w.write_all(b"n")?;
                w.write_all(&(name.len() as u128).to_be_bytes())?;
                w.write_all(name.as_bytes())?;
                item.write_to(w)?;
            }
            HashItem::List(items) => {
                w.write_all(b"l")?;
                w.write_all(&(items.len() as u128).to_be_bytes())?;

                for item in items {
                    item.write_to(w)?;
                }
            }
            HashItem::Raw(raw) => {
                w.write_all(raw)?;
            }
        }

        Ok(())
    }

    pub fn to_raw(self) -> HashItem<'static> {
        let mut raw = Vec::new();
        self.write_to(&mut raw).unwrap();

        HashItem::Raw(raw)
    }

    pub fn hash(&self) -> String {
        let mut state = Sha256::new();
        self.write_to(&mut state).unwrap();
        format!("{:x}", state.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_item() {
        let item = HashItem::List(vec![
            HashItem::named("a", "a"),
            HashItem::named("b", "b"),
            HashItem::List(vec![HashItem::named("c", "c"), HashItem::named("d", "d")]),
        ]);

        // If you change the hash algorithm, this is is expected to break.
        //
        // Make sure this is intended though.
        assert_eq!(
            item.hash(),
            "1c292ae624b26d6cdb8a0f874d05f52fa48d57e7fa34bac895c1590f08189f4a"
        );
    }

    #[test]
    fn test_hash_item_injective() {
        let a = HashItem::List(vec![HashItem::List(vec![
            HashItem::named("a", "a"),
            HashItem::named("b", "b"),
        ])]);
        let b = HashItem::List(vec![HashItem::named("a", "a"), HashItem::named("b", "b")]);

        assert_ne!(a.hash(), b.hash());
    }
}
