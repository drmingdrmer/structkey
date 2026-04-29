//! `#[derive(KeyCodec)]` on a unit struct must be rejected.
//!
//! Unit structs (`struct Foo;`) have no fields. Use `struct Foo {}` if a
//! field-less but otherwise normal struct is needed.

use structkey::KeyCodec;

#[derive(KeyCodec)]
struct Bad;

fn main() {}
