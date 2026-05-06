//! Integration tests for `#[derive(Codec)]`.
//!
//! Lives under `tests/` so it links `structkey` as an external crate,
//! exercising the derive the same way a downstream user would.

use std::marker::PhantomData;

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
    structkey::testing::assert_round_trip(
        Triple {
            a: 1,
            b: "hello world".to_string(),
            c: 2,
        },
        "tri/1/hello%20world/2",
    );
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

// Compile-fail coverage for invalid derive attributes lives in
// `derive_compile_fail.rs`; this file covers successful expansion paths.

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

// Enum with all three variant shapes: named-field, tuple, unit. Each
// variant is encoded as `<lowercased-ident>/<fields...>`. Verifies that
// the discriminant segment lands first and that variants with different
// segment counts round-trip correctly.
#[derive(Debug, PartialEq, Eq, Codec)]
enum Shape {
    Named { id: u64, label: String },
    Tuple(u64, String),
    Unit,
}

#[test]
fn enum_named_variant_round_trip() {
    round_trip(
        Shape::Named {
            id: 7,
            label: "alice".to_string(),
        },
        "named/7/alice",
    );
}

#[test]
fn enum_tuple_variant_round_trip() {
    round_trip(Shape::Tuple(42, "bob".to_string()), "tuple/42/bob");
}

#[test]
fn enum_unit_variant_round_trip() {
    round_trip(Shape::Unit, "unit");
}

#[test]
fn enum_segment_count_includes_discriminant() {
    assert_eq!(
        3,
        Shape::Named {
            id: 1,
            label: "x".to_string()
        }
        .segment_count()
    );
    assert_eq!(3, Shape::Tuple(1, "x".to_string()).segment_count());
    assert_eq!(1, Shape::Unit.segment_count());
}

#[test]
fn enum_decode_unknown_discriminant() {
    let mut parser = Parser::new("nope");
    let result = Shape::decode_key(&mut parser);
    match result {
        Err(Error::InvalidSegment { expect, got, .. }) => {
            assert_eq!("named|tuple|unit", expect);
            assert_eq!("nope", got);
        }
        other => panic!("expected InvalidSegment, got {:?}", other),
    }
}

// Enum with `#[codec(raw)]` on a variant field. Lowercasing the variant
// ident yields `tag` so that's the discriminant; the field is pushed
// raw, skipping the percent-escape on the space.
#[derive(Debug, PartialEq, Eq, Codec)]
enum Tagged {
    Tag {
        #[codec(raw)]
        name: String,
    },
}

#[test]
fn enum_raw_variant_field_skips_escape() {
    let v = Tagged::Tag {
        name: "a b".to_string(),
    };
    round_trip(v, "tag/a b");
}

// Single-variant enum — verifies the macro accepts an enum with no
// catch-all needed in practice (unknown discriminants still error).
#[derive(Debug, PartialEq, Eq, Codec)]
enum Singleton {
    Only(u64),
}

#[test]
fn enum_singleton_round_trip() {
    round_trip(Singleton::Only(99), "only/99");
}

// Multi-word variants default to `snake_case`. Acronyms collapse
// (`UDF` -> `udf`), and an acronym/word boundary still gets a separator
// (`XMLParser` -> `xml_parser`). Use `#[codec(rename = "...")]` to
// pick a non-snake separator.
#[allow(clippy::upper_case_acronyms)] // testing acronym snake_case behavior
#[derive(Debug, PartialEq, Eq, Codec)]
enum MultiWord {
    TwoWords { a: u64, b: u64 },
    ThreeWordVariant(u64),
    UDF,
    XMLParser(u64),
}

#[test]
fn enum_multi_word_variant_uses_snake_case() {
    round_trip(MultiWord::TwoWords { a: 1, b: 2 }, "two_words/1/2");
    round_trip(MultiWord::ThreeWordVariant(7), "three_word_variant/7");
    round_trip(MultiWord::UDF, "udf");
    round_trip(MultiWord::XMLParser(9), "xml_parser/9");
}

// Per-variant `#[codec(rename = "...")]` -- the escape hatch when the
// default lowercase is wrong (existing wire format, want a separator,
// or the variant's lowercase collides with another variant).
#[derive(Debug, PartialEq, Eq, Codec)]
enum WithRename {
    #[codec(rename = "two-words")]
    TwoWords {
        a: u64,
        b: u64,
    },

    #[codec(rename = "X")]
    Alpha(u64),

    Plain,
}

#[test]
fn enum_rename_overrides_default_tag() {
    round_trip(WithRename::TwoWords { a: 1, b: 2 }, "two-words/1/2");
    round_trip(WithRename::Alpha(7), "X/7");
    // Variants without `rename` still get the default lowercase.
    round_trip(WithRename::Plain, "plain");
}

// Generic struct with a `PhantomData` marker. The marker type carries
// no runtime data and gets silently skipped: not encoded, not decoded,
// not counted -- which means it also can't be required to impl Codec.
#[derive(Debug, PartialEq, Eq, Codec)]
struct WithMarker<R> {
    id: u64,
    _p: PhantomData<R>,
}

// Marker type with no Codec impl. The derive must NOT auto-add an
// `R: Codec` bound; if it did, this struct would fail to compile.
#[derive(Debug, PartialEq, Eq)]
struct NoCodecMarker;

#[test]
fn struct_with_phantom_data_skips_field() {
    round_trip(
        WithMarker::<NoCodecMarker> {
            id: 42,
            _p: PhantomData,
        },
        "42",
    );
    assert_eq!(
        1,
        WithMarker::<NoCodecMarker> {
            id: 1,
            _p: PhantomData,
        }
        .segment_count(),
    );
}

// Two phantom fields side by side. Tests the all-skipped path of
// segment_count_expr (must emit `0`, not an empty `+` chain).
#[derive(Debug, PartialEq, Eq, Codec)]
struct OnlyPhantom<A, B> {
    _a: PhantomData<A>,
    _b: PhantomData<B>,
}

#[test]
fn struct_with_only_phantom_fields_has_zero_segments() {
    let v: OnlyPhantom<NoCodecMarker, NoCodecMarker> = OnlyPhantom {
        _a: PhantomData,
        _b: PhantomData,
    };
    assert_eq!(0, v.segment_count());
    let s = v.encode_key(Builder::new()).done();
    assert_eq!("", s);
}

// Phantom data also works on enum variants -- both named-field and
// tuple shape -- so an enum can carry a marker without forcing its
// param to impl Codec.
#[derive(Debug, PartialEq, Eq, Codec)]
enum WithPhantom<R> {
    Named { id: u64, _p: PhantomData<R> },
    Tuple(u64, PhantomData<R>),
}

#[test]
fn enum_phantom_field_skipped_in_named_and_tuple_variants() {
    round_trip(
        WithPhantom::<NoCodecMarker>::Named {
            id: 7,
            _p: PhantomData,
        },
        "named/7",
    );
    round_trip(
        WithPhantom::<NoCodecMarker>::Tuple(99, PhantomData),
        "tuple/99",
    );
}

// `#[derive(StructKey)]` alone — it implies `#[derive(Codec)]` and
// emits both impls, so users don't need to list both. Field attributes
// from the `codec` namespace (`#[codec(raw)]`, `#[codec(rename)]` on
// variants) still work.
#[derive(Debug, PartialEq, Eq, StructKey)]
#[structkey(prefix = "user")]
struct DerivedKey {
    user_id: u64,
    name: String,
}

#[test]
fn struct_key_derive_provides_prefix_and_round_trips() {
    structkey::testing::assert_round_trip(
        DerivedKey {
            user_id: 7,
            name: "alice".to_string(),
        },
        "user/7/alice",
    );
}

// StructKey on a generic struct with a PhantomData marker. Verifies
// `split_for_impl()` carries `R` through cleanly and that no `R: Codec`
// bound is synthesized (the marker doesn't impl Codec yet this
// compiles).
#[derive(Debug, PartialEq, Eq, StructKey)]
#[structkey(prefix = "tagged")]
struct DerivedKeyGeneric<R> {
    id: u64,
    _p: PhantomData<R>,
}

#[test]
fn struct_key_derive_works_on_generic_with_marker() {
    structkey::testing::assert_round_trip(
        DerivedKeyGeneric::<NoCodecMarker> {
            id: 99,
            _p: PhantomData,
        },
        "tagged/99",
    );
}

#[test]
fn enum_rename_appears_in_decode_error_message() {
    let mut parser = Parser::new("nope");
    let err = WithRename::decode_key(&mut parser).unwrap_err();
    match err {
        Error::InvalidSegment { expect, got, .. } => {
            // `expect` lists the *effective* tags (post-rename), not
            // the raw idents -- otherwise the error would mislead
            // anyone trying to construct a valid key.
            assert_eq!("two-words|X|plain", expect);
            assert_eq!("nope", got);
        }
        other => panic!("expected InvalidSegment, got {:?}", other),
    }
}
