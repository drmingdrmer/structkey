//! `#[key_codec(<unknown>)]` must be rejected so typos don't silently
//! bypass the raw path. This fixture uses `rwa` -- a plausible typo of
//! `raw`.

use structkey::KeyCodec;

#[derive(KeyCodec)]
struct Bad {
    #[key_codec(rwa)]
    name: String,
}

fn main() {}
