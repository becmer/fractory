// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use bincode::{Decode, Encode};

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode)]
pub struct Request {
    pub kind: u32,
    pub data: String,
}

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode)]
pub enum Response {
    Echo(Request),
}
