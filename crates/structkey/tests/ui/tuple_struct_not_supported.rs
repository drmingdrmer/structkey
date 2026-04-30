//! `#[derive(Codec)]` on a tuple struct must be rejected.

use structkey::Codec;

#[derive(Codec)]
struct Bad(u64, String);

fn main() {}
