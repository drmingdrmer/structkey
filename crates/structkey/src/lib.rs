//! Encode structured Rust types as `/`-separated, percent-escaped string keys.
//!
//! Designed as a string-key namespace abstraction for KV stores: a typed
//! key becomes a deterministic, human-readable string, and the same
//! string round-trips back to the original value.
//!
//! # Quick start
//!
//! ```
//! use structkey::KeyCodec;
//! use structkey::StructKey;
//!
//! #[derive(Debug, PartialEq, Eq, KeyCodec)]
//! struct UserSession {
//!     user_id: u64,
//!     session: String,
//! }
//!
//! impl StructKey for UserSession {
//!     const PREFIX: &'static str = "session";
//! }
//!
//! let s = UserSession {
//!     user_id: 42,
//!     session: "abc def".to_string(),
//! };
//! assert_eq!("session/42/abc%20def", s.to_string_key());
//!
//! let parsed = UserSession::from_str_key("session/42/abc%20def").unwrap();
//! assert_eq!(s, parsed);
//! ```
//!
//! # Concepts
//!
//! - [`KeyCodec`] — how one logical field is encoded / decoded.
//! - [`StructKey`] — a constant `PREFIX` plus a sequence of fields,
//!   producing the canonical `prefix/field1/.../fieldN` form.
//! - [`Raw`] — a `String` newtype whose codec skips percent-escaping.
//!   Available as a field type or via `#[key_codec(raw)]` on a
//!   `String` field of a `#[derive(KeyCodec)]` struct.
//! - [`DirName`] — a print-only view that drops trailing segments.
//!   Useful for forming a parent prefix or list-prefix from any
//!   structured key. Decoding a `DirName` is intentionally an error.
//! - [`KeyBuilder`] / [`KeyParser`] — the byte-level building blocks.
//! - [`KeyError`] — the crate's only error type.
//!
//! # Escape policy
//!
//! `String::encode_key` treats every byte outside `[A-Za-z0-9_]` as
//! special and emits it as `%XX` in lowercase hex. The segment
//! separator `/` is therefore always escaped, and the parser can split
//! unambiguously.
//!
//! # Cargo features
//!
//! - `derive` *(default)* — enables `#[derive(KeyCodec)]`. Disable with
//!   `default-features = false` to avoid the proc-macro toolchain.

// Alias the crate as `::structkey` so the derive macro can use the same
// path (`::structkey::KeyCodec`, etc.) for in-crate doctests and
// external consumers, instead of branching on a `crate::*` form that
// only works in one context.
extern crate self as structkey;

mod dir_name;
mod helper;
mod key_builder;
mod key_codec;
mod key_error;
mod key_parser;
mod raw;
mod struct_key;

pub use dir_name::DirName;
pub use key_builder::KeyBuilder;
pub use key_codec::KeyCodec;
pub use key_error::KeyError;
pub use key_parser::KeyParser;
pub use raw::Raw;
pub use struct_key::StructKey;
#[cfg(feature = "derive")]
pub use structkey_derive::KeyCodec;
