//! `#[derive(Codec)]` on a union must be rejected.

use structkey::Codec;

#[derive(Codec)]
union Bad {
    a: u64,
    b: u32,
}

fn main() {}
