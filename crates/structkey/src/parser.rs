use std::iter::Peekable;
use std::str::Split;

use crate::Error;
use crate::helper::decode_id;
use crate::helper::unescape;
use crate::helper::unescape_specified;

/// A helper for parsing a string key into structured key.
pub struct Parser<'s> {
    str_key: &'s str,
    /// The index of the next item to return.
    i: usize,
    /// The index of char for the next return.
    index: usize,
    elements: Peekable<Split<'s, char>>,
}

impl<'s> Parser<'s> {
    /// Create a new parser, split the str key by delimiter `/`.
    pub fn new(source: &'s str) -> Self {
        Self {
            str_key: source,
            i: 0,
            index: 0,
            elements: source.split('/').peekable(),
        }
    }

    /// Similar to `new()` but expect the specified prefix as the first part.
    pub fn new_prefixed(source: &'s str, prefix: &str) -> Result<Self, Error> {
        let mut s = Self::new(source);
        s.next_literal(prefix)?;
        Ok(s)
    }

    pub fn source(&self) -> &str {
        self.str_key
    }

    /// Get the index of the last returned element.
    ///
    /// If no element is returned, it will panic.
    pub fn index(&self) -> usize {
        self.i - 1
    }

    /// Peek the next element in raw `&str` and expect it to be `expect`, without unescaping or decoding.
    pub fn peek_literal(&mut self, expect: &str) -> Result<(), Error> {
        let next = self
            .elements
            .peek()
            .ok_or_else(|| Error::WrongNumberOfSegments {
                expect: self.i,
                got: self.str_key.to_string(),
            })?;

        if *next != expect {
            return Err(Error::InvalidSegment {
                i: self.i,
                expect: expect.to_string(),
                got: next.to_string(),
            });
        }

        Ok(())
    }

    /// Pop the next element in raw `&str`, without unescaping or decoding.
    ///
    /// If there is no more element, it returns Error::WrongNumberOfSegments.
    pub fn next_raw(&mut self) -> Result<&'s str, Error> {
        let elt = self.elements.next();

        if let Some(s) = elt {
            self.i += 1;
            self.index += s.len() + 1;
            Ok(s)
        } else {
            Err(Error::WrongNumberOfSegments {
                // Expected length
                expect: self.i + 1,
                got: self.str_key.to_string(),
            })
        }
    }

    /// Pop the next element, unescape it, and require non-empty.
    ///
    /// Returns `Error::EmptySegment` if the segment unescapes to an empty
    /// string. Callers that need a typed non-empty wrapper can promote the
    /// returned `String` themselves.
    pub fn next_nonempty(&mut self) -> Result<String, Error> {
        let elt = self.next_raw()?;
        let s = unescape(elt)?;
        if s.is_empty() {
            return Err(Error::EmptySegment { i: self.i - 1 });
        }
        Ok(s)
    }

    /// Pop the next element and unescape it.
    pub fn next_str(&mut self) -> Result<String, Error> {
        let elt = self.next_raw()?;
        let s = unescape(elt)?;
        Ok(s)
    }

    /// Pop the next element and parse it into a u64.
    pub fn next_u64(&mut self) -> Result<u64, Error> {
        let elt = self.next_raw()?;
        decode_id(elt)
    }

    /// Pop the next element that equals `expect`.
    ///
    /// If there is no more element, or the popped element is different, it returns Error::WrongNumberOfSegments.
    pub fn next_literal(&mut self, expect: &str) -> Result<(), Error> {
        let ith = self.i;
        let elt = self.next_raw()?;

        if elt != expect {
            return Err(Error::InvalidSegment {
                i: ith,
                expect: expect.to_string(),
                got: elt.to_string(),
            });
        }

        Ok(())
    }

    /// Return trailing raw string that is not processed.
    pub fn tail_raw(&mut self) -> Result<&str, Error> {
        let index = self.index;
        let _ = self.next_raw()?;

        // Exhaust the rest of the elements
        while self.next_raw().is_ok() {}

        Ok(&self.str_key[index..])
    }

    /// Finish parsing, if there are ore elements left, it returns Error::WrongNumberOfSegments
    pub fn done(&mut self) -> Result<(), Error> {
        let ith = self.i;
        let elt = self.elements.next();

        if elt.is_some() {
            return Err(Error::WrongNumberOfSegments {
                expect: ith,
                got: self.str_key.to_string(),
            });
        }
        Ok(())
    }

    /// Re-export unescape()
    pub fn unescape(s: &str) -> Result<String, Error> {
        unescape(s)
    }

    /// Re-export unescape()
    pub fn unescape_specified(s: &str, chars: &[u8]) -> Result<String, Error> {
        unescape_specified(s, chars)
    }
}

#[cfg(test)]
mod tests {
    use crate::Error;
    use crate::parser::Parser;

    #[test]
    fn test_parser_new_prefixed() -> anyhow::Result<()> {
        {
            let s = "_foo/bar%20-/123";
            let mut kp = Parser::new_prefixed(s, "_foo")?;
            assert_eq!(Ok("bar%20-"), kp.next_raw());
        }

        {
            let s = "_foo/bar%20-/123";
            let kp = Parser::new_prefixed(s, "bar");
            assert!(kp.is_err());
        }

        Ok(())
    }

    #[test]
    fn test_parser_next() -> anyhow::Result<()> {
        let s = "_foo/bar%20-/123";

        let mut kp = Parser::new(s);
        assert_eq!(Ok("_foo"), kp.next_raw());
        assert_eq!(Ok("bar%20-"), kp.next_raw());
        assert_eq!(Ok("123"), kp.next_raw());
        assert!(kp.next_raw().is_err());

        Ok(())
    }

    #[test]
    fn test_parser_next_nonempty() -> anyhow::Result<()> {
        let s = "_foo/bar%21-//123";

        let mut kp = Parser::new(s);
        assert_eq!(Ok("_foo".to_string()), kp.next_nonempty());
        assert_eq!(Ok("bar!-".to_string()), kp.next_nonempty());
        assert_eq!(Err(Error::EmptySegment { i: 2 }), kp.next_nonempty());

        Ok(())
    }

    #[test]
    fn test_parser_next_str() -> anyhow::Result<()> {
        let s = "_foo/bar%21-/123";

        let mut kp = Parser::new(s);
        assert_eq!(Ok("_foo".to_string()), kp.next_str());
        assert_eq!(Ok("bar!-".to_string()), kp.next_str());

        Ok(())
    }

    #[test]
    fn test_parser_next_u64() -> anyhow::Result<()> {
        let s = "_foo/bar%20-/123";

        let mut kp = Parser::new(s);
        assert!(kp.next_u64().is_err());
        assert!(kp.next_u64().is_err());
        assert_eq!(Ok(123), kp.next_u64());

        Ok(())
    }

    #[test]
    fn test_parser_next_literal() -> anyhow::Result<()> {
        let s = "_foo/bar%20-/123";

        let mut kp = Parser::new(s);
        assert!(kp.next_literal("_foo").is_ok());
        assert!(kp.next_literal("bar%20-").is_ok());
        assert!(kp.next_literal("foo").is_err());
        assert!(kp.next_literal("123").is_err(), "already consumed");

        Ok(())
    }

    #[test]
    fn test_parser_tail() -> anyhow::Result<()> {
        let s = "_foo/bar%20-/123";

        {
            let mut kp = Parser::new(s);
            assert_eq!(Ok(s), kp.tail_raw());
            assert!(kp.done().is_ok());
        }
        {
            let mut kp = Parser::new(s);
            kp.next_raw()?;
            assert_eq!(Ok("bar%20-/123"), kp.tail_raw());
            assert!(kp.done().is_ok());
        }
        {
            let mut kp = Parser::new(s);
            kp.next_raw()?;
            kp.next_raw()?;
            assert_eq!(Ok("123"), kp.tail_raw());
            assert!(kp.done().is_ok());
        }
        {
            let mut kp = Parser::new(s);
            kp.next_raw()?;
            kp.next_raw()?;
            kp.next_raw()?;
            assert!(kp.tail_raw().is_err());
            assert!(kp.done().is_ok());
        }

        Ok(())
    }

    #[test]
    fn test_parser_done() -> anyhow::Result<()> {
        let s = "_foo/bar%20-/123";

        let mut kp = Parser::new(s);
        assert!(kp.done().is_err());
        assert!(kp.next_literal("bar%20-").is_ok());
        assert!(kp.next_literal("123").is_ok());
        assert!(kp.done().is_ok());

        Ok(())
    }

    #[test]
    fn test_parser_peek_literal() -> anyhow::Result<()> {
        let s = "_foo/bar%20-/123";

        let mut kp = Parser::new(s);
        assert!(kp.peek_literal("_foo").is_ok());
        assert_eq!(
            kp.peek_literal("bar").unwrap_err().to_string(),
            "Expect 0-th segment to be 'bar', but: '_foo'"
        );
        assert!(kp.next_literal("_foo").is_ok());

        Ok(())
    }
}
