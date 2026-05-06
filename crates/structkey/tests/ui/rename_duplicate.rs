//! Effective enum discriminants must be unique. A renamed variant can
//! otherwise encode to the same tag as another variant and decode as the
//! first match arm.

use structkey::Codec;

#[derive(Codec)]
enum Duplicate {
    Foo,
    #[codec(rename = "foo")]
    Bar,
}

fn main() {}
