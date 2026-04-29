//! Trait for whole structured keys.
//!
//! While [`KeyCodec`] describes how a single field is encoded into and
//! decoded from a string, `StructKey` describes the whole key: it adds a
//! constant `PREFIX` and exact-segment-count framing on top of `KeyCodec`,
//! producing the canonical `prefix/field1/field2/.../fieldN` form.

use crate::KeyBuilder;
use crate::KeyCodec;
use crate::KeyError;
use crate::KeyParser;

/// A structured key: a constant `PREFIX` plus a sequence of `KeyCodec` fields.
///
/// The default `to_string_key` / `from_str_key` impls compose `KeyCodec` with
/// `PREFIX`-framing and require the parser to be fully consumed, so a key
/// that decodes successfully has exactly the segments the type expects --
/// no more, no fewer.
pub trait StructKey: KeyCodec
where Self: Sized
{
    /// The literal first segment of every encoded key of this type.
    const PREFIX: &'static str;

    /// Encode `self` into the canonical `prefix/...` string form.
    fn to_string_key(&self) -> String {
        let b = KeyBuilder::new_prefixed(Self::PREFIX);
        self.encode_key(b).done()
    }

    /// Decode a string into a structured key.
    ///
    /// Fails if the prefix does not match, if any field fails to decode, or
    /// if extra trailing segments remain after all fields have been consumed.
    fn from_str_key(s: &str) -> Result<Self, KeyError> {
        let mut p = KeyParser::new_prefixed(s, Self::PREFIX)?;
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

    impl KeyCodec for Empty {
        fn encode_key(&self, b: KeyBuilder) -> KeyBuilder {
            b
        }

        fn decode_key(_p: &mut KeyParser) -> Result<Self, KeyError> {
            Ok(Empty)
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

    impl KeyCodec for Pair {
        fn encode_key(&self, b: KeyBuilder) -> KeyBuilder {
            self.b.encode_key(self.a.encode_key(b))
        }

        fn decode_key(p: &mut KeyParser) -> Result<Self, KeyError> {
            Ok(Pair {
                a: u64::decode_key(p)?,
                b: String::decode_key(p)?,
            })
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
}
