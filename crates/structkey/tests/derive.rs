//! Integration tests for `#[derive(Codec)]`.
//!
//! Lives under `tests/` so it links `structkey` as an external crate,
//! exercising the derive the same way a downstream user would.

use structkey::Builder;
use structkey::Codec;
use structkey::DirName;
use structkey::Error;
use structkey::Parser;
use structkey::Raw;
use structkey::StructKey;

fn round_trip<T>(value: T, expected: &str)
where T: Codec + PartialEq + std::fmt::Debug {
    let encoded = value.encode_key(Builder::new()).done();
    assert_eq!(expected, encoded, "encode mismatch");

    let mut parser = Parser::new(&encoded);
    let decoded = T::decode_key(&mut parser).expect("decode");
    assert_eq!(value, decoded, "round-trip mismatch");
}

// Basic two-field struct.
#[derive(Debug, PartialEq, Eq, Codec)]
struct Pair {
    name: String,
    id: u64,
}

#[test]
fn pair_round_trip() {
    round_trip(
        Pair {
            name: "alice".to_string(),
            id: 42,
        },
        "alice/42",
    );
}

#[test]
fn pair_escapes_separator_in_string() {
    // `/` in a String field must be escaped so the parser doesn't split on it.
    round_trip(
        Pair {
            name: "a/b".to_string(),
            id: 1,
        },
        "a%2fb/1",
    );
}

#[test]
fn pair_segment_count_sums_fields() {
    let p = Pair {
        name: "x".to_string(),
        id: 1,
    };
    assert_eq!(2, p.segment_count());
}

// Single-field struct -- exercises the `let_and_return` shape.
#[derive(Debug, PartialEq, Eq, Codec)]
struct Only {
    value: u64,
}

#[test]
fn only_single_field_round_trip() {
    round_trip(Only { value: 99 }, "99");
}

// Empty struct -- exercises the `unused_variables` allow on `n` and `p`.
#[derive(Debug, PartialEq, Eq, Codec)]
struct Empty {}

#[test]
fn empty_round_trip() {
    round_trip(Empty {}, "");
}

#[test]
fn empty_segment_count_is_zero() {
    assert_eq!(0, Empty {}.segment_count());
}

// Three-field struct, mixed types -- the typical "structured key" shape.
#[derive(Debug, PartialEq, Eq, Codec)]
struct Triple {
    a: u64,
    b: String,
    c: u64,
}

impl StructKey for Triple {
    const PREFIX: &'static str = "tri";
}

#[test]
fn triple_via_struct_key_to_string() {
    let k = Triple {
        a: 1,
        b: "hello world".to_string(),
        c: 2,
    };
    assert_eq!("tri/1/hello%20world/2", k.to_string_key());
    assert_eq!(k, Triple::from_str_key(&k.to_string_key()).unwrap());
}

#[test]
fn triple_works_under_dir_name() {
    let k = Triple {
        a: 1,
        b: "hello world".to_string(),
        c: 2,
    };
    let dir = DirName::new(k); // level 1 -> drop one segment
    assert_eq!("tri/1/hello%20world", dir.to_string_key());
}

// Raw field -- bypasses percent-escaping.
#[derive(Debug, PartialEq, Eq, Codec)]
struct WithRaw {
    a: u64,
    #[codec(raw)]
    b: String,
}

#[test]
fn raw_field_skips_escape() {
    let k = WithRaw {
        a: 1,
        b: "a b".to_string(), // a space would normally escape to %20
    };
    let s = k.encode_key(Builder::new()).done();
    assert_eq!("1/a b", s);

    // Round-trip works because the parser splits on `/`, and `a b`
    // contains no `/`.
    let mut parser = Parser::new(&s);
    let decoded = WithRaw::decode_key(&mut parser).unwrap();
    assert_eq!(k, decoded);
}

// Builder's segment budget is honoured: capping the builder emits fewer
// segments than the full encoding, without involving DirName.
#[test]
fn encode_key_honours_builder_budget() {
    let k = Triple {
        a: 1,
        b: "x".to_string(),
        c: 2,
    };
    let s = k.encode_key(Builder::new().limit_segments(2)).done();
    assert_eq!("1/x", s);

    let s = k.encode_key(Builder::new().limit_segments(1)).done();
    assert_eq!("1", s);

    let s = k.encode_key(Builder::new().limit_segments(0)).done();
    assert_eq!("", s);
}

// Unknown attribute option fails compilation -- can't test directly
// here without trybuild, but the `codec(raw)` path is exercised
// above. A trybuild compile-fail suite can be added later if needed.

// `Error` import is exercised by `decode_key`'s signature in the
// generated impl; this assertion documents that the error type is
// reachable from here.
#[allow(dead_code)]
fn _assert_error_is_reachable() {
    fn _t() -> Result<(), Error> {
        Ok(())
    }
}

// Decode failure: too few segments. `Pair` expects two; one is provided.
// The derived `decode_key` propagates the parser's segment-count error
// instead of producing a half-decoded value.
#[test]
fn decode_too_few_segments() {
    let mut parser = Parser::new("alice");
    let result = Pair::decode_key(&mut parser);
    assert!(
        matches!(result, Err(Error::WrongNumberOfSegments { .. })),
        "expected WrongNumberOfSegments, got {:?}",
        result,
    );
}

// Decode failure: non-numeric input for a `u64` field. The `u64` codec
// refuses to silently coerce; the derived impl surfaces the error.
#[test]
fn decode_non_numeric_for_u64_field() {
    let mut parser = Parser::new("alice/notnum");
    let result = Pair::decode_key(&mut parser);
    assert!(result.is_err(), "expected decode error, got {:?}", result);
}

// Generic struct with a user-supplied `T: Codec` bound. Verifies
// `split_for_impl()` propagates generics and the existing where-clause.
// The derive does not auto-inject `T: Codec`; users declare it on the
// struct.
#[derive(Debug, PartialEq, Eq, Codec)]
struct Generic<T: Codec> {
    id: T,
    suffix: String,
}

#[test]
fn generic_with_string_param() {
    round_trip(
        Generic::<String> {
            id: "abc".to_string(),
            suffix: "x".to_string(),
        },
        "abc/x",
    );
}

// `Raw` used directly as a field type (not via `#[codec(raw)]`). This
// exercises a different branch in the derive — the field is encoded
// through `Raw`'s own `Codec` impl without the `Raw::from_ref` shim.
#[derive(Debug, PartialEq, Eq, Codec)]
struct WithRawField {
    id: u64,
    tag: Raw,
}

#[test]
fn raw_field_type_round_trip() {
    round_trip(
        WithRawField {
            id: 7,
            tag: Raw("hello world".to_string()),
        },
        "7/hello world",
    );
}
