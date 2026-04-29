//! Truncated-prefix view over a `StructKey`.
//!
//! `DirName<K>` wraps a key and a `level` count. `to_string_key()` encodes
//! the key as usual and then drops `level` trailing segments. So
//! `DirName::new(k)` (level 1) prints the parent of `k`; level 2 drops two
//! segments; level 0 returns the full encoding.
//!
//! Truncation is clamped at the prefix: a level that exceeds the segment
//! count returns just `K::PREFIX`.

use crate::KeyBuilder;
use crate::KeyCodec;
use crate::KeyError;
use crate::KeyParser;
use crate::StructKey;

/// A view of a `StructKey` truncated to its first `n` segments, where
/// `n` is the encoded segment count minus `level`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirName<K> {
    key: K,
    level: usize,
}

impl<K> DirName<K> {
    /// Create a `DirName` at the default level of 1 (one segment dropped).
    pub fn new(key: K) -> Self {
        DirName { key, level: 1 }
    }

    /// Create a `DirName` at an explicit truncation level.
    pub fn new_with_level(key: K, level: usize) -> Self {
        DirName { key, level }
    }

    /// Update the truncation level in place.
    pub fn with_level(&mut self, level: usize) -> &mut Self {
        self.level = level;
        self
    }

    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn into_key(self) -> K {
        self.key
    }
}

impl<K> DirName<K>
where K: StructKey
{
    /// `to_string_key()` with a trailing `/`. Convenient for building
    /// list-prefix queries against KV stores that scan by string range.
    pub fn dir_name_with_slash(&self) -> String {
        let prefix = self.to_string_key();
        format!("{}/", prefix)
    }
}

// `DirName` is a transformation over a structured key, not a sequence of
// fields, so it has no meaningful per-field codec. The impl exists only
// to satisfy the `KeyCodec` supertrait of `StructKey`; both methods panic
// because nothing in this crate calls them on a `DirName` -- the
// `StructKey` impl below overrides `to_string_key` / `from_str_key` so
// the default routes through `encode_key` / `decode_key` are bypassed.
impl<K: StructKey> KeyCodec for DirName<K> {
    fn encode_key(&self, _b: KeyBuilder) -> KeyBuilder {
        unimplemented!("DirName has no field-level encoding")
    }

    fn decode_key(_p: &mut KeyParser) -> Result<Self, KeyError> {
        unimplemented!("DirName has no field-level decoding")
    }
}

impl<K: StructKey> StructKey for DirName<K> {
    const PREFIX: &'static str = K::PREFIX;

    fn to_string_key(&self) -> String {
        let k = self.key.to_string_key();
        // `rsplitn(n, ...)` returns at most `n` parts, splitting from the
        // right; `.last()` is the un-split remainder. With `n = level + 1`
        // that remainder has `level` fewer segments than the input.
        k.rsplitn(self.level + 1, '/').last().unwrap().to_string()
    }

    /// Decode `s` as a `K` and wrap it at level 0.
    ///
    /// At level 0, `to_string_key()` reproduces the input verbatim. Callers
    /// that want a truncated form can adjust the level afterwards.
    fn from_str_key(s: &str) -> Result<Self, KeyError> {
        let k = K::from_str_key(s)?;
        Ok(DirName::new_with_level(k, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Foo {
        a: u64,
        b: String,
        c: u64,
    }

    impl KeyCodec for Foo {
        fn encode_key(&self, b: KeyBuilder) -> KeyBuilder {
            self.c.encode_key(self.b.encode_key(self.a.encode_key(b)))
        }

        fn decode_key(p: &mut KeyParser) -> Result<Self, KeyError> {
            Ok(Foo {
                a: u64::decode_key(p)?,
                b: String::decode_key(p)?,
                c: u64::decode_key(p)?,
            })
        }
    }

    impl StructKey for Foo {
        const PREFIX: &'static str = "pref";
    }

    #[test]
    fn from_str_key_round_trips_at_level_zero() {
        let d = DirName::<Foo>::from_str_key("pref/9/x/8").unwrap();
        assert_eq!(
            Foo {
                a: 9,
                b: "x".to_string(),
                c: 8,
            },
            d.into_key()
        );
    }

    #[test]
    fn level_truncates_segments() {
        let k = Foo {
            a: 1,
            b: "b".to_string(),
            c: 2,
        };

        let mut dir = DirName::new(k);
        assert_eq!("pref/1/b", dir.to_string_key());

        dir.with_level(0);
        assert_eq!("pref/1/b/2", dir.to_string_key());

        dir.with_level(2);
        assert_eq!("pref/1", dir.to_string_key());

        dir.with_level(3);
        assert_eq!("pref", dir.to_string_key());

        dir.with_level(4);
        assert_eq!(
            "pref",
            dir.to_string_key(),
            "level above depth clamps to prefix"
        );
    }

    #[test]
    fn recursive_nesting_drops_one_level_each() {
        let k = Foo {
            a: 1,
            b: "b".to_string(),
            c: 2,
        };

        let dir = DirName::new(k);
        assert_eq!("pref/1/b", dir.to_string_key());

        let dir = DirName::new(dir);
        assert_eq!("pref/1", dir.to_string_key());

        let dir = DirName::new(dir);
        assert_eq!("pref", dir.to_string_key());

        let dir = DirName::new(dir);
        assert_eq!("pref", dir.to_string_key(), "root dir clamps");
    }

    #[test]
    fn dir_name_with_slash_appends_separator() {
        let k = Foo {
            a: 1,
            b: "b".to_string(),
            c: 2,
        };
        let dir = DirName::new(k);
        assert_eq!("pref/1/b/", dir.dir_name_with_slash());
    }
}
