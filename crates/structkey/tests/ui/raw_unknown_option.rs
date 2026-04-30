//! `#[codec(<unknown>)]` must be rejected so typos don't silently
//! bypass the raw path. This fixture uses `rwa` -- a plausible typo of
//! `raw`.

use structkey::Codec;

#[derive(Codec)]
struct Bad {
    #[codec(rwa)]
    name: String,
}

fn main() {}
