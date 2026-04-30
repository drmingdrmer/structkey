//! `#[derive(Codec)]` on an enum must be rejected.

use structkey::Codec;

#[derive(Codec)]
enum Bad {
    A,
    B(u64),
}

fn main() {}
