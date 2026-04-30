//! `#[derive(Codec)]` on a unit struct must be rejected.
//!
//! Unit structs (`struct Foo;`) have no fields. Use `struct Foo {}` if a
//! field-less but otherwise normal struct is needed.

use structkey::Codec;

#[derive(Codec)]
struct Bad;

fn main() {}
