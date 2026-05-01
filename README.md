# structkey

Encode structured Rust types as `/`-separated, percent-escaped string keys — designed as a string-key namespace abstraction for KV stores. A typed key becomes a deterministic, human-readable string; the same string round-trips back to the original value.

```rust
use structkey::StructKey;

#[derive(Debug, PartialEq, Eq, StructKey)]
#[structkey(prefix = "session")]
struct UserSession {
    user_id: u64,
    session: String,
}

let s = UserSession { user_id: 42, session: "abc def".to_string() };
assert_eq!("session/42/abc%20def", s.to_string_key());

let parsed = UserSession::from_str_key("session/42/abc%20def").unwrap();
assert_eq!(s, parsed);
```

## What it does

A `StructKey` is encoded as `prefix/field1/field2/.../fieldN`. Each field implements `Codec`, which says how a single value is pushed onto a `Builder` and recovered from a `Parser`. The `Builder` owns the segment budget internally, so codec impls don't have to thread a counter through fields. The crate ships:

- **`#[derive(Codec)]`** for structs with named fields and for enums (named, tuple, and unit variants). Fields are encoded in declaration order; for enums, the variant adds a leading discriminant segment. Use this for types that are *parts* of a key — e.g. an enum embedded inside a larger struct key.
- **`#[derive(StructKey)]`** + `#[structkey(prefix = "...")]` for top-level keys. Implies `#[derive(Codec)]` (don't combine, that's a duplicate-impl error). Prefix must be non-empty and free of `/`.
- **Built-in `Codec` impls** for `String` (percent-escapes special bytes), `u64` / `u32` (decimal), and `()` (zero-segment, useful for prefix-only keys).
- **`Raw`**, a `String` newtype whose `Codec` skips escaping. Use directly or via `#[codec(raw)]` on a `String` field. The caller is responsible for ensuring the value contains no `/`.
- **`DirName<K>`**, a print-only view that drops the trailing `level` segments — handy for forming a parent prefix or list-prefix from any structured key. Decoding a `DirName<K>` from a string is intentionally an error; decode `K` directly and wrap.

## Enums

Variants encode as `<discriminant>/<fields...>`. The discriminant is the variant name in `snake_case` by default — multi-word variants get a real separator (`TwoWords` → `two_words`, `RowAccessPolicy` → `row_access_policy`); pure acronyms collapse (`UDF` → `udf`); and acronym/word boundaries split (`XMLParser` → `xml_parser`).

```rust
#[derive(Debug, PartialEq, Eq, Codec)]
enum Object {
    Database { db_id: u64 },
    #[codec(rename = "two-words")]
    TwoWords(u64, String),
    Unit,
}
```

`#[codec(rename = "...")]` overrides the discriminant per variant. The value must be non-empty and must not contain `/` — both rejected at compile time.

## PhantomData

Fields whose type's last path segment is `PhantomData` are silently skipped: not encoded, not decoded, and contribute nothing to `segment_count`. This lets the derive cover marker-typed structs like `DataId<R> { id: u64, _p: PhantomData<R> }` without forcing the marker `R` to implement `Codec`.

Detection is by the last path segment, so `PhantomData<R>`, `std::marker::PhantomData<R>`, and `core::marker::PhantomData<R>` are all recognised. A user-defined type literally named `PhantomData` would be a false positive — rename it or hand-write the impl.

## Escape policy

`String::encode_key` treats every byte outside `[A-Za-z0-9_]` as special and emits it as `%XX` in lowercase hex. The segment separator `/` is therefore always escaped, and the parser can split unambiguously. See [percent-encoding](https://en.wikipedia.org/wiki/Percent-encoding) for context.

## Cargo features

- `derive` *(default)* — enables `#[derive(Codec)]`, re-exported from `structkey-derive`. Disable with `default-features = false` if you want to avoid the proc-macro toolchain.

## License

Apache-2.0.
