//! Trait for whole structured keys.
//!
//! While [`Codec`] describes how a single field is encoded into and
//! decoded from a string, `StructKey` describes the whole key: it adds a
//! constant `PREFIX` and exact-segment-count framing on top of `Codec`,
//! producing the canonical `prefix/field1/field2/.../fieldN` form.

use crate::Builder;
use crate::Codec;
use crate::Error;
use crate::Parser;

/// A structured key: a constant `PREFIX` plus a sequence of `Codec` fields.
///
/// The default `to_string_key` / `from_str_key` impls compose `Codec` with
/// `PREFIX`-framing and require the parser to be fully consumed, so a key
/// that decodes successfully has exactly the segments the type expects --
/// no more, no fewer.
pub trait StructKey: Codec
where Self: Sized
{
    /// The literal first segment of every encoded key of this type.
    const PREFIX: &'static str;

    /// Encode `self` into the canonical `prefix/...` string form.
    ///
    /// Uses `self.segment_count()` as the segment cap, so an impl whose
    /// `encode_key` would emit more than `segment_count` claims still
    /// produces an output with the declared segment count.
    fn to_string_key(&self) -> String {
        let b = Builder::new_prefixed(Self::PREFIX);
        self.encode_key(b, self.segment_count()).done()
    }

    /// Decode a string into a structured key.
    ///
    /// Fails if the prefix does not match, if any field fails to decode, or
    /// if extra trailing segments remain after all fields have been consumed.
    fn from_str_key(s: &str) -> Result<Self, Error> {
        let mut p = Parser::new_prefixed(s, Self::PREFIX)?;
        let k = Self::decode_key(&mut p)?;
        p.done()?;

        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Empty;

    impl Codec for Empty {
        fn encode_key(&self, b: Builder, _n: usize) -> Builder {
            b
        }

        fn decode_key(_p: &mut Parser) -> Result<Self, Error> {
            Ok(Empty)
        }

        fn segment_count(&self) -> usize {
            0
        }
    }

    impl StructKey for Empty {
        const PREFIX: &'static str = "empty";
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Pair {
        a: u64,
        b: String,
    }

    impl Codec for Pair {
        fn encode_key(&self, b: Builder, n: usize) -> Builder {
            let b = self.a.encode_key(b, n);
            let n = n.saturating_sub(self.a.segment_count());
            self.b.encode_key(b, n)
        }

        fn decode_key(p: &mut Parser) -> Result<Self, Error> {
            Ok(Pair {
                a: u64::decode_key(p)?,
                b: String::decode_key(p)?,
            })
        }

        fn segment_count(&self) -> usize {
            self.a.segment_count() + self.b.segment_count()
        }
    }

    impl StructKey for Pair {
        const PREFIX: &'static str = "p";
    }

    #[test]
    fn round_trip_prefix_only() {
        let k = Empty;
        let s = k.to_string_key();
        assert_eq!(s, "empty");
        assert_eq!(Empty::from_str_key(&s).unwrap(), k);
    }

    #[test]
    fn round_trip_multi_field() {
        let k = Pair {
            a: 7,
            b: "hello world".to_string(),
        };
        let s = k.to_string_key();
        assert_eq!(s, "p/7/hello%20world");
        assert_eq!(Pair::from_str_key(&s).unwrap(), k);
    }

    #[test]
    fn from_str_key_rejects_wrong_prefix() {
        assert!(Empty::from_str_key("other").is_err());
    }

    #[test]
    fn from_str_key_rejects_extra_segments() {
        assert!(Empty::from_str_key("empty/extra").is_err());
    }

    /// Fixture used to verify the cap. Its `encode_key` would emit two
    /// segments at `n = usize::MAX`, but `segment_count` reports 1.
    /// This represents a buggy or out-of-sync impl; `to_string_key`
    /// must still produce an output consistent with the declared count.
    #[derive(Debug, PartialEq, Eq)]
    struct Underclaim {
        a: u64,
        b: u64,
    }

    impl Codec for Underclaim {
        fn encode_key(&self, b: Builder, n: usize) -> Builder {
            let b = self.a.encode_key(b, n);
            let n = n.saturating_sub(self.a.segment_count());
            self.b.encode_key(b, n)
        }

        fn decode_key(p: &mut Parser) -> Result<Self, Error> {
            Ok(Underclaim {
                a: u64::decode_key(p)?,
                b: u64::decode_key(p)?,
            })
        }

        fn segment_count(&self) -> usize {
            1
        }
    }

    impl StructKey for Underclaim {
        const PREFIX: &'static str = "uc";
    }

    #[test]
    fn to_string_key_caps_emitted_segments_at_segment_count() {
        let k = Underclaim { a: 1, b: 2 };
        // segment_count reports 1, so to_string_key emits PREFIX + 1
        // segment ("uc/1"), not "uc/1/2" -- even though encode_key would
        // gladly push both fields if given a larger `n`.
        assert_eq!("uc/1", k.to_string_key());
    }
}
