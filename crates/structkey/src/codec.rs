use crate::Builder;
use crate::Error;
use crate::Parser;

/// Encode or decode one logical field of a structured key.
///
/// Implementers describe how a single value is appended to a
/// [`Builder`] and recovered from a [`Parser`]. A struct-level
/// `Codec` is composed by calling each field's codec in order, so a
/// hand-written or derived impl looks like a sequence of `encode_key` /
/// `decode_key` calls — one per field, in declaration order.
///
/// The builder carries its own segment budget (see
/// [`Builder::limit_segments`]). Once the budget reaches zero, further
/// `push_*` calls become no-ops, so a `Codec` impl can pipe pushes
/// through without checking a counter — [`DirName`] sets the cap on the
/// builder before delegating to the inner codec.
///
/// [`DirName`]: crate::DirName
pub trait Codec {
    /// Encode fields of the structured key into a key builder.
    ///
    /// Each push consumes one slot of the builder's segment budget.
    /// Aggregates simply chain pushes — they don't need to know the cap.
    fn encode_key(&self, b: Builder) -> Builder;

    /// Decode fields of the structured key from a key parser.
    fn decode_key(parser: &mut Parser) -> Result<Self, Error>
    where Self: Sized;

    /// Number of segments this value contributes when fully encoded.
    ///
    /// Must equal the number of segments
    /// `encode_key` pushes onto an unbounded builder. Truncation logic
    /// (e.g. `DirName`) and `to_string_key` rely on this count being
    /// exact.
    fn segment_count(&self) -> usize;
}

mod impls {
    use crate::Builder;
    use crate::Codec;
    use crate::Error;
    use crate::Parser;

    impl Codec for String {
        fn encode_key(&self, b: Builder) -> Builder {
            b.push_str(self)
        }

        fn decode_key(p: &mut Parser) -> Result<Self, Error>
        where Self: Sized {
            let s = p.next_str()?;
            Ok(s)
        }

        fn segment_count(&self) -> usize {
            1
        }
    }

    impl Codec for u64 {
        fn encode_key(&self, b: Builder) -> Builder {
            b.push_u64(*self)
        }

        fn decode_key(p: &mut Parser) -> Result<Self, Error>
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
    impl Codec for u32 {
        fn encode_key(&self, b: Builder) -> Builder {
            b.push_u64(*self as u64)
        }

        fn decode_key(p: &mut Parser) -> Result<Self, Error>
        where Self: Sized {
            let n = p.next_u64()?;
            u32::try_from(n).map_err(|_| Error::InvalidId {
                s: n.to_string(),
                reason: format!("value {} does not fit in u32", n),
            })
        }

        fn segment_count(&self) -> usize {
            1
        }
    }

    impl Codec for () {
        fn encode_key(&self, b: Builder) -> Builder {
            b
        }

        fn decode_key(_p: &mut Parser) -> Result<Self, Error>
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
    use crate::Builder;
    use crate::Codec;
    use crate::Parser;

    #[test]
    fn test_string_round_trip() {
        let s = "hello world".to_string();
        let encoded = s.encode_key(Builder::new()).done();
        let mut p = Parser::new(&encoded);
        let decoded = String::decode_key(&mut p).unwrap();
        assert_eq!(s, decoded);
    }

    #[test]
    fn test_string_with_special_chars() {
        let s = "a/b%c".to_string();
        let encoded = s.encode_key(Builder::new()).done();
        let mut p = Parser::new(&encoded);
        let decoded = String::decode_key(&mut p).unwrap();
        assert_eq!(s, decoded);
    }

    #[test]
    fn test_u64_round_trip() {
        for v in [0u64, 1, 42, u64::MAX] {
            let encoded = v.encode_key(Builder::new()).done();
            let mut p = Parser::new(&encoded);
            let decoded = u64::decode_key(&mut p).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn test_unit_round_trip() {
        let encoded = ().encode_key(Builder::new()).done();
        assert_eq!(encoded, "");
        let mut p = Parser::new(&encoded);
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

    #[test]
    fn test_encode_with_zero_budget_pushes_nothing() {
        let b = Builder::new().limit_segments(0);
        let b = "abc".to_string().encode_key(b);
        let b = 99u64.encode_key(b);
        let b = 99u32.encode_key(b);
        let b = ().encode_key(b);
        assert_eq!("", b.done());
    }
}
