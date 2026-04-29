//! `#[derive(KeyCodec)]` on a union must be rejected.

use structkey::KeyCodec;

#[derive(KeyCodec)]
union Bad {
    a: u64,
    b: u32,
}

fn main() {}
