//! A helper for building a string key from a structured key.
//!
//! `Builder` carries an optional segment budget so callers can cap how
//! many segments their pushes append. Once the budget reaches zero, the
//! `push_*` methods become no-ops. This lets aggregate `Codec` impls
//! stay sequence-of-pushes without threading a segment count through
//! every field.

use crate::helper::escape;
use crate::helper::escape_specified;

pub struct Builder {
    buf: Vec<u8>,
    /// Number of segments written so far. This cannot be derived from
    /// `buf.is_empty()` because an empty string is still a real segment.
    segments_written: usize,
    /// Maximum number of further segments `push_*` may append. Default
    /// `usize::MAX` is effectively unlimited; `0` makes pushes no-ops.
    budget: usize,
}

impl Builder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            segments_written: 0,
            budget: usize::MAX,
        }
    }

    /// Construct a builder seeded with `prefix` as its first segment.
    ///
    /// The prefix is pushed under the default unlimited budget, so it's
    /// never suppressed by a later `limit_segments`.
    pub fn new_prefixed(prefix: &str) -> Self {
        Self::new().push_raw(prefix)
    }

    /// Tighten the segment budget for subsequent `push_*` calls.
    ///
    /// Always takes the minimum of the current budget and `n`. Nesting
    /// `limit_segments` calls therefore only ever shrinks the cap, which
    /// lets `DirName` clamp its inner key safely even when the outer
    /// caller already constrained the builder.
    pub fn limit_segments(mut self, n: usize) -> Self {
        self.budget = self.budget.min(n);
        self
    }

    /// Push a segment without escaping.
    ///
    /// Returns unchanged if the segment budget is exhausted.
    pub fn push_raw(mut self, s: &str) -> Self {
        if self.budget == 0 {
            return self;
        }
        self.budget -= 1;
        if self.segments_written > 0 {
            // `/`
            self.buf.push(0x2f);
        }
        self.segments_written += 1;
        self.buf.extend_from_slice(s.as_bytes());
        self
    }

    /// Push a percent-escaped segment.
    ///
    /// Returns unchanged if the segment budget is exhausted.
    pub fn push_str(self, s: &str) -> Self {
        self.push_raw(&escape(s))
    }

    /// Push a u64 segment as decimal.
    ///
    /// Returns unchanged if the segment budget is exhausted.
    pub fn push_u64(self, n: u64) -> Self {
        self.push_raw(&format!("{}", n))
    }

    pub fn done(self) -> String {
        String::from_utf8(self.buf).unwrap()
    }

    /// Re-export escape()
    pub fn escape(s: &str) -> String {
        escape(s)
    }

    /// Re-export escape_specified()
    pub fn escape_specified(s: &str, chars: &[u8]) -> String {
        escape_specified(s, chars)
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::Builder;

    #[test]
    fn test_builder() -> anyhow::Result<()> {
        let s = Builder::new_prefixed("_foo")
            .push_str("a b")
            .push_u64(5)
            .push_raw("a b")
            .done();

        assert_eq!("_foo/a%20b/5/a b", s);
        Ok(())
    }

    #[test]
    fn limit_segments_caps_pushes() {
        let s = Builder::new_prefixed("p")
            .limit_segments(2)
            .push_u64(1)
            .push_u64(2)
            .push_u64(3) // dropped
            .done();
        assert_eq!("p/1/2", s);
    }

    #[test]
    fn empty_first_segment_still_separates_following_segments() {
        let s = Builder::new().push_str("").push_u64(1).done();
        assert_eq!("/1", s);

        let s = Builder::new_prefixed("p").push_str("").push_u64(1).done();
        assert_eq!("p//1", s);
    }

    #[test]
    fn limit_segments_zero_drops_all_data() {
        // The prefix is seeded before the budget, so it survives.
        let s = Builder::new_prefixed("p")
            .limit_segments(0)
            .push_u64(1)
            .done();
        assert_eq!("p", s);
    }

    #[test]
    fn limit_segments_takes_minimum() {
        // Tighter outer cap wins over looser inner cap.
        let s = Builder::new_prefixed("p")
            .limit_segments(1)
            .limit_segments(5)
            .push_u64(1)
            .push_u64(2)
            .done();
        assert_eq!("p/1", s);

        // Tighter inner cap wins over looser outer cap.
        let s = Builder::new_prefixed("p")
            .limit_segments(5)
            .limit_segments(1)
            .push_u64(1)
            .push_u64(2)
            .done();
        assert_eq!("p/1", s);
    }
}
