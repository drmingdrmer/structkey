//! `#[codec(rename = "...")]` rejects empty values and values containing
//! the segment separator, since either would silently corrupt the wire
//! format (empty -> empty segment; `/` -> the parser would split inside
//! the discriminant).

use structkey::Codec;

#[derive(Codec)]
enum Empty {
    #[codec(rename = "")]
    A,
}

#[derive(Codec)]
enum WithSlash {
    #[codec(rename = "a/b")]
    A,
}

fn main() {}
