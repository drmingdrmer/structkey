//! `#[derive(KeyCodec)]` on an enum must be rejected.

use structkey::KeyCodec;

#[derive(KeyCodec)]
enum Bad {
    A,
    B(u64),
}

fn main() {}
