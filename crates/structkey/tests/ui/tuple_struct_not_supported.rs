//! `#[derive(KeyCodec)]` on a tuple struct must be rejected.

use structkey::KeyCodec;

#[derive(KeyCodec)]
struct Bad(u64, String);

fn main() {}
