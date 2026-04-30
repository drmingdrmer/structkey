//! Truncated-prefix view over a `StructKey`.
//!
//! `DirName<K>` wraps a key and a `level` count. `to_string_key()` encodes
//! the key as usual and then drops `level` trailing segments. So
//! `DirName::new(k)` (level 1) prints the parent of `k`; level 2 drops two
//! segments; level 0 returns the full encoding.
//!
//! Truncation is clamped at the prefix: a level that exceeds the segment
//! count returns just `K::PREFIX`.

use crate::Builder;
use crate::Codec;
use crate::Error;
use crate::Parser;
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

impl<K: Codec> Codec for DirName<K> {
    /// Encode at most `min(n, self.segment_count())` segments by asking
    /// the inner `K` for that many. K honours the limit, so trailing
    /// fields are never encoded.
    fn encode_key(&self, b: Builder, n: usize) -> Builder {
        self.key.encode_key(b, n.min(self.segment_count()))
    }

    /// `DirName` is a print-only view of a key; it does not round-trip
    /// through a string, so decoding always fails.
    ///
    /// `from_str_key` therefore returns this error too. Callers that need
    /// a `K` from a string call `K::from_str_key` directly, then wrap
    /// the result in `DirName::new` / `new_with_level`.
    fn decode_key(_p: &mut Parser) -> Result<Self, Error> {
        Err(Error::NotDecodable {
            type_name: std::any::type_name::<Self>(),
        })
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

    impl Codec for Foo {
        fn encode_key(&self, b: Builder, n: usize) -> Builder {
            let b = self.a.encode_key(b, n);
            let n = n.saturating_sub(self.a.segment_count());
            let b = self.b.encode_key(b, n);
            let n = n.saturating_sub(self.b.segment_count());
            self.c.encode_key(b, n)
        }

        fn decode_key(p: &mut Parser) -> Result<Self, Error> {
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
    fn from_str_key_returns_not_decodable() {
        let err = DirName::<Foo>::from_str_key("pref/9/x/8").unwrap_err();
        assert!(matches!(err, Error::NotDecodable { .. }));
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
    fn encode_key_self_clamps_to_segment_count() {
        let k = Foo {
            a: 1,
            b: "b".to_string(),
            c: 2,
        };
        let dir = DirName::new(k); // level 1, segment_count 2

        // The caller asks for 99 segments; DirName self-clamps to its own
        // segment_count (2), so K is called with n = 2 and emits exactly
        // 2 segments. This is what makes DirName safe to embed as a field
        // in a larger key: a parent that passes a wide `n` cannot make
        // DirName over-emit.
        let s = dir.encode_key(Builder::new(), 99).done();
        assert_eq!("1/b", s);
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
