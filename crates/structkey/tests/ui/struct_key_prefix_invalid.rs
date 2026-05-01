//! `#[derive(StructKey)]` validates the `prefix` container attribute at
//! compile time:
//! - missing entirely (the trait requires `PREFIX`, the derive can't
//!   guess one)
//! - empty string (would round-trip an unprefixed key, defeating the
//!   `from_str_key` prefix check)
//! - contains `/` (would split into multiple segments since the prefix
//!   is emitted via `push_raw`)

use structkey::StructKey;

#[derive(StructKey)]
struct Missing {
    id: u64,
}

#[derive(StructKey)]
#[structkey(prefix = "")]
struct Empty {
    id: u64,
}

#[derive(StructKey)]
#[structkey(prefix = "a/b")]
struct WithSlash {
    id: u64,
}

fn main() {}
