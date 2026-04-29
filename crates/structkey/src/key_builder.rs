//! A helper for building a string key from a structured key

use crate::helper::escape;
use crate::helper::escape_specified;

pub struct KeyBuilder {
    buf: Vec<u8>,
}

impl KeyBuilder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn new_prefixed(prefix: &str) -> Self {
        let b = Self::new();
        b.push_raw(prefix)
    }

    pub fn push_raw(mut self, s: &str) -> Self {
        if !self.buf.is_empty() {
            // `/`
            self.buf.push(0x2f);
        }

        self.buf.extend_from_slice(s.as_bytes());
        self
    }

    pub fn push_str(self, s: &str) -> Self {
        self.push_raw(&escape(s))
    }

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
    use crate::key_builder::KeyBuilder;

    #[test]
    fn test_key_builder() -> anyhow::Result<()> {
        let s = KeyBuilder::new_prefixed("_foo")
            .push_str("a b")
            .push_u64(5)
            .push_raw("a b")
            .done();

        assert_eq!("_foo/a%20b/5/a b", s);
        Ok(())
    }
}
