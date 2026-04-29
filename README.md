# structkey

Encode structured Rust types as `/`-separated, percent-escaped string keys — designed as a string-key namespace abstraction for KV stores. A typed key becomes a deterministic, human-readable string; the same string round-trips back to the original value.

```rust
use structkey::{KeyCodec, StructKey};

#[derive(Debug, PartialEq, Eq, KeyCodec)]
struct UserSession {
    user_id: u64,
    session: String,
}

impl StructKey for UserSession {
    const PREFIX: &'static str = "session";
}

let s = UserSession { user_id: 42, session: "abc def".to_string() };
assert_eq!("session/42/abc%20def", s.to_string_key());

let parsed = UserSession::from_str_key("session/42/abc%20def").unwrap();
assert_eq!(s, parsed);
```

## What it does

A `StructKey` is encoded as `prefix/field1/field2/.../fieldN`. Each field implements `KeyCodec`, which says how a single value is pushed onto a `KeyBuilder` and recovered from a `KeyParser`. The crate ships:

- **`#[derive(KeyCodec)]`** for structs with named fields. Fields are encoded in declaration order; the derive sums each field's `segment_count` and threads the segment limit `n` through them so partial encoding works.
- **Built-in `KeyCodec` impls** for `String` (percent-escapes special bytes), `u64` / `u32` (decimal), and `()` (zero-segment, useful for prefix-only keys).
- **`Raw`**, a `String` newtype whose `KeyCodec` skips escaping. Use directly or via `#[key_codec(raw)]` on a `String` field. The caller is responsible for ensuring the value contains no `/`.
- **`DirName<K>`**, a print-only view that drops the trailing `level` segments — handy for forming a parent prefix or list-prefix from any structured key. Decoding a `DirName<K>` from a string is intentionally an error; decode `K` directly and wrap.

## Escape policy

`String::encode_key` treats every byte outside `[A-Za-z0-9_]` as special and emits it as `%XX` in lowercase hex. The segment separator `/` is therefore always escaped, and the parser can split unambiguously. See [percent-encoding](https://en.wikipedia.org/wiki/Percent-encoding) for context.

## Cargo features

- `derive` *(default)* — enables `#[derive(KeyCodec)]`, re-exported from `structkey-derive`. Disable with `default-features = false` if you want to avoid the proc-macro toolchain.

## License

Apache-2.0.
