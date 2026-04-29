// Copyright 2025 Zhang Yanpo
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::string::FromUtf8Error;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),

    #[error("Expect {i}-th segment to be '{expect}', but: '{got}'")]
    InvalidSegment {
        i: usize,
        expect: String,
        got: String,
    },

    #[error("Expect {i}-th segment to be non-empty")]
    EmptySegment { i: usize },

    #[error("Expect {expect} segments, but: '{got}'")]
    WrongNumberOfSegments { expect: usize, got: String },

    #[error("Expect at least {expect} segments, but {actual} segments found")]
    AtleastSegments { expect: usize, actual: usize },

    #[error("Invalid id string: '{s}': {reason}")]
    InvalidId { s: String, reason: String },

    #[error("Unknown key prefix: '{prefix}'")]
    UnknownPrefix { prefix: String },

    #[error("Invalid percent-encoded sequence at byte {pos} in '{input}'")]
    InvalidEscape { pos: usize, input: String },
}
