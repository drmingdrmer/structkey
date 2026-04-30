//! `#[codec(raw)]` on a non-`String` field must fail to compile,
//! because the macro emits `Raw::from_ref(&self.<field>)` which is
//! typed `&String -> &Raw`. A `u64` field produces a clear type-mismatch
//! error pointing at the generated call site.

use structkey::Codec;

#[derive(Codec)]
struct Bad {
    #[codec(raw)]
    id: u64,
}

fn main() {}
