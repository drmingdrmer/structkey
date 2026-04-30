//! A newtype wrapper around `String` whose `Codec` impl bypasses escaping.
//!
//! `String`'s `Codec` impl uses [`Builder::push_str`] /
//! [`Parser::next_str`], which percent-escape special bytes (notably the
//! segment separator `/`) so the encoded form round-trips safely.
//!
//! `Raw`'s `Codec` impl uses [`Builder::push_raw`] /
//! [`Parser::next_raw`] — the value is written byte-for-byte. The caller
//! is responsible for ensuring the value contains no `/`; otherwise the
//! decoder will split on it and produce a wrong-segment-count error.
//!
//! [`Builder::push_str`]: super::Builder::push_str
//! [`Parser::next_str`]: super::Parser::next_str
//! [`Builder::push_raw`]: super::Builder::push_raw
//! [`Parser::next_raw`]: super::Parser::next_raw

use crate::Builder;
use crate::Codec;
use crate::Error;
use crate::Parser;

/// A `String` newtype whose `Codec` impl skips escaping.
///
/// Use either directly as a field type, or via `#[codec(raw)]` on a
/// `String` field of a `#[derive(Codec)]` struct — the derive macro then
/// routes that field through `Raw`'s impl.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Raw(pub String);

impl Raw {
    /// View a `&String` as a `&Raw` without copying.
    ///
    /// Used by the `#[derive(Codec)]` macro to route a `String` field
    /// through `Raw`'s `Codec` impl.
    pub fn from_ref(s: &String) -> &Self {
        // SAFETY: `Raw` is `#[repr(transparent)]` over `String`, so the two
        // types share an identical memory layout, and casting `*const String`
        // to `*const Raw` is sound.
        unsafe { &*(s as *const String as *const Raw) }
    }

    /// Consume the wrapper and return the inner `String`.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Codec for Raw {
    fn encode_key(&self, b: Builder, n: usize) -> Builder {
        if n == 0 { b } else { b.push_raw(&self.0) }
    }

    fn decode_key(p: &mut Parser) -> Result<Self, Error>
    where Self: Sized {
        Ok(Raw(p.next_raw()?.to_string()))
    }

    fn segment_count(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_ref_is_zero_cost() {
        let s = "hello".to_string();
        let r: &Raw = Raw::from_ref(&s);
        // Same address — confirms no copy.
        assert_eq!(&s as *const String as *const Raw, r as *const Raw);
        assert_eq!(r.0, "hello");
    }

    #[test]
    fn raw_skips_escape() {
        // A space would normally be escaped to `%20` by `push_str`.
        let r = Raw("a b".to_string());
        let s = r.encode_key(Builder::new(), usize::MAX).done();
        assert_eq!("a b", s);

        let mut p = Parser::new(&s);
        let decoded = Raw::decode_key(&mut p).unwrap();
        assert_eq!(r, decoded);
    }
}
