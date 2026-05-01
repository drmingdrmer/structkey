//! Test helpers for application crates that build keys with [`StructKey`].
//!
//! These are intended for use inside `#[cfg(test)]` modules — they panic
//! on assertion failure and trade brevity for production fitness.
//!
//! [`StructKey`]: crate::StructKey

use std::fmt::Debug;

use crate::StructKey;

/// Assert that `key` encodes to `expected` and decodes back to itself.
///
/// Equivalent to writing the two `assert_eq!`s by hand:
///
/// ```text
/// assert_eq!(key.to_string_key(), expected);
/// assert_eq!(T::from_str_key(expected).unwrap(), key);
/// ```
///
/// `expected` is `impl ToString` so callers can pass either a `&str`
/// literal or a built-up `String`. The function consumes `key` since
/// tests typically construct one fresh per assertion.
///
/// # Panics
///
/// - Encode mismatch (`key.to_string_key() != expected`).
/// - `from_str_key` returns `Err` on the expected string.
/// - The decoded value doesn't equal the original `key`.
pub fn assert_round_trip<T>(key: T, expected: impl ToString)
where T: StructKey + PartialEq + Debug {
    let expected = expected.to_string();

    let encoded = key.to_string_key();
    assert_eq!(encoded, expected, "encode mismatch for {:?}", key,);

    let decoded = T::from_str_key(&expected)
        .unwrap_or_else(|e| panic!("decode of {:?} failed: {}", expected, e));
    assert_eq!(decoded, key, "round-trip mismatch for {:?}", expected,);
}
