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

impl<K: KeyCodec> KeyCodec for DirName<K> {
    /// Encode the inner key and push only the first
    /// `key.segment_count() - level` segments onto `b`. Encoding produces
    /// a complete K string first; truncation then slices off the tail
    /// without an extra allocation.
    fn encode_key(&self, b: KeyBuilder) -> KeyBuilder {
        let kept = self.segment_count();
        if kept == 0 {
            return b;
        }
        let k_encoded = self.key.encode_key(KeyBuilder::new()).done();
        // Position of the `kept`-th `/`, or end-of-string if there are
        // fewer separators than that. Slicing up to that index keeps
        // exactly `kept` segments.
        let cut_at = k_encoded
            .match_indices('/')
            .nth(kept - 1)
            .map(|(i, _)| i)
            .unwrap_or(k_encoded.len());
        b.push_raw(&k_encoded[..cut_at])
    }

    /// Consume `K`'s segments from the parser and wrap the result at
    /// level 0 -- i.e. a `DirName` whose `to_string_key` reproduces the
    /// just-parsed input. Callers that want a truncated form adjust the
    /// level afterwards.
    fn decode_key(p: &mut KeyParser) -> Result<Self, KeyError> {
        let k = K::decode_key(p)?;
        Ok(DirName::new_with_level(k, 0))
    }

    fn segment_count(&self) -> usize {
        self.key.segment_count().saturating_sub(self.level)
    }
}

impl<K: StructKey> StructKey for DirName<K> {
    const PREFIX: &'static str = K::PREFIX;
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

        fn segment_count(&self) -> usize {
            self.a.segment_count() + self.b.segment_count() + self.c.segment_count()
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

    #[test]
    fn segment_count_subtracts_level() {
        let k = Foo {
            a: 1,
            b: "b".to_string(),
            c: 2,
        };
        // K has 3 segments. Level subtracts; clamp at 0.
        let mut dir = DirName::new_with_level(k, 0);
        assert_eq!(3, dir.segment_count());
        dir.with_level(1);
        assert_eq!(2, dir.segment_count());
        dir.with_level(3);
        assert_eq!(0, dir.segment_count());
        dir.with_level(99);
        assert_eq!(
            0,
            dir.segment_count(),
            "level above K's segment count clamps to 0"
        );
    }
}
