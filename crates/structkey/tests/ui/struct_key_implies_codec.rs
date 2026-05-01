//! Combining `#[derive(Codec, StructKey)]` is an error: StructKey
//! already emits the Codec impl, so adding Codec produces a duplicate
//! `impl Codec` -- locked in here so the rule doesn't quietly drift.

use structkey::Codec;
use structkey::StructKey;

#[derive(Codec, StructKey)]
#[structkey(prefix = "x")]
struct Both {
    id: u64,
}

fn main() {}
