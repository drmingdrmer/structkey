use crate::KeyBuilder;
use crate::KeyError;
use crate::KeyParser;

/// Encode or decode one logical field of a structured key.
///
/// Implementers describe how a single value is appended to a
/// [`KeyBuilder`] and recovered from a [`KeyParser`]. A struct-level
/// `KeyCodec` is composed by calling each field's codec in order, so a
/// hand-written or derived impl looks like a sequence of `encode_key` /
/// `decode_key` calls — one per field, in declaration order.
pub trait KeyCodec {
    /// Encode fields of the structured key into a key builder.
    fn encode_key(&self, b: KeyBuilder) -> KeyBuilder;

    /// Decode fields of the structured key from a key parser.
    fn decode_key(parser: &mut KeyParser) -> Result<Self, KeyError>
    where Self: Sized;

    /// Number of segments this value contributes when encoded.
    ///
    /// Must equal the number of segments `encode_key` pushes onto the
    /// builder. Truncation logic (e.g. `DirName`) and any future
    /// segment-bounded encoding rely on this count being exact.
    fn segment_count(&self) -> usize;
}

mod impls {
    use crate::KeyBuilder;
    use crate::KeyCodec;
    use crate::KeyError;
    use crate::KeyParser;

    impl KeyCodec for String {
        fn encode_key(&self, b: KeyBuilder) -> KeyBuilder {
            b.push_str(self)
        }

        fn decode_key(p: &mut KeyParser) -> Result<Self, KeyError>
        where Self: Sized {
            let s = p.next_str()?;
            Ok(s)
        }

        fn segment_count(&self) -> usize {
            1
        }
    }

    impl KeyCodec for u64 {
        fn encode_key(&self, b: KeyBuilder) -> KeyBuilder {
            b.push_u64(*self)
        }

        fn decode_key(p: &mut KeyParser) -> Result<Self, KeyError>
        where Self: Sized {
            let s = p.next_u64()?;
            Ok(s)
        }

        fn segment_count(&self) -> usize {
            1
        }
    }

    /// Wire format is identical to `u64`: a decimal segment.
    /// Decode rejects values that exceed `u32::MAX` rather than silently
    /// truncating, so a key crafted with an out-of-range integer fails fast
    /// instead of round-tripping to a different value.
    impl KeyCodec for u32 {
        fn encode_key(&self, b: KeyBuilder) -> KeyBuilder {
            b.push_u64(*self as u64)
        }

        fn decode_key(p: &mut KeyParser) -> Result<Self, KeyError>
        where Self: Sized {
            let n = p.next_u64()?;
            u32::try_from(n).map_err(|_| KeyError::InvalidId {
                s: n.to_string(),
                reason: format!("value {} does not fit in u32", n),
            })
        }

        fn segment_count(&self) -> usize {
            1
        }
    }

    impl KeyCodec for () {
        fn encode_key(&self, b: KeyBuilder) -> KeyBuilder {
            b
        }

        fn decode_key(_p: &mut KeyParser) -> Result<Self, KeyError>
        where Self: Sized {
            Ok(())
        }

        fn segment_count(&self) -> usize {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::KeyBuilder;
    use crate::KeyCodec;
    use crate::KeyParser;

    #[test]
    fn test_string_round_trip() {
        let s = "hello world".to_string();
        let encoded = s.encode_key(KeyBuilder::new()).done();
        let mut p = KeyParser::new(&encoded);
        let decoded = String::decode_key(&mut p).unwrap();
        assert_eq!(s, decoded);
    }

    #[test]
    fn test_string_with_special_chars() {
        let s = "a/b%c".to_string();
        let encoded = s.encode_key(KeyBuilder::new()).done();
        let mut p = KeyParser::new(&encoded);
        let decoded = String::decode_key(&mut p).unwrap();
        assert_eq!(s, decoded);
    }

    #[test]
    fn test_u64_round_trip() {
        for v in [0u64, 1, 42, u64::MAX] {
            let encoded = v.encode_key(KeyBuilder::new()).done();
            let mut p = KeyParser::new(&encoded);
            let decoded = u64::decode_key(&mut p).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn test_unit_round_trip() {
        let encoded = ().encode_key(KeyBuilder::new()).done();
        assert_eq!(encoded, "");
        let mut p = KeyParser::new(&encoded);
        <()>::decode_key(&mut p).unwrap();
    }

    #[test]
    fn test_segment_counts() {
        assert_eq!(1, "anything".to_string().segment_count());
        assert_eq!(1, 0u64.segment_count());
        assert_eq!(1, u64::MAX.segment_count());
        assert_eq!(1, 0u32.segment_count());
        assert_eq!(0, ().segment_count());
    }
}
