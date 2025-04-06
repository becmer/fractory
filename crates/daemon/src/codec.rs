// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::fmt;

use bincode::{Decode, Encode, config::Configuration, decode_from_slice, encode_into_std_write};
use tokio::io;
use tokio_util::{
    bytes::{Buf, BufMut, BytesMut},
    codec::{Decoder, Encoder},
};

use crate::msg::{Request, Response};

const RESERVE_CAPACITY: usize = 1024;
const HEADER_LEN: usize = size_of::<u32>();

pub struct ClientCodec {
    config: Configuration,
}

impl ClientCodec {
    pub fn new() -> Self {
        Self {
            config: bincode::config::standard(),
        }
    }
}

impl Encoder<Request> for ClientCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Request, dst: &mut BytesMut) -> io::Result<()> {
        encode(item, dst, self.config)
    }
}

impl Decoder for ClientCodec {
    type Item = Response;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        decode(src, self.config)
    }
}

impl fmt::Debug for ClientCodec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ClientCodec").finish_non_exhaustive()
    }
}

pub struct ServerCodec {
    config: Configuration,
}

impl ServerCodec {
    pub fn new() -> Self {
        Self {
            config: bincode::config::standard(),
        }
    }
}

impl Encoder<Response> for ServerCodec {
    type Error = io::Error;

    fn encode(&mut self, item: Response, dst: &mut BytesMut) -> io::Result<()> {
        encode(item, dst, self.config)
    }
}

impl Decoder for ServerCodec {
    type Item = Request;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> io::Result<Option<Self::Item>> {
        decode(src, self.config)
    }
}

impl fmt::Debug for ServerCodec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ServerCodec").finish_non_exhaustive()
    }
}

fn encode<T: Encode>(item: T, dst: &mut BytesMut, config: Configuration) -> io::Result<()> {
    dst.reserve(RESERVE_CAPACITY);
    dst.put_u32_le(0);
    let writer = &mut dst.writer();
    match encode_into_std_write(item, writer, config) {
        Ok(len) => {
            let header = (len as u32).to_le_bytes();
            dst[..HEADER_LEN].copy_from_slice(&header);
            Ok(())
        }
        Err(e) => Err(io::Error::new(io::ErrorKind::InvalidInput, e)),
    }
}

fn decode<T: Decode<()>>(src: &mut BytesMut, config: Configuration) -> io::Result<Option<T>> {
    if src.len() < HEADER_LEN {
        return Ok(None);
    }

    let mut header = [0u8; HEADER_LEN];
    header.copy_from_slice(&src[..HEADER_LEN]);
    let len = u32::from_le_bytes(header) as usize;

    if src.len() < HEADER_LEN + len {
        return Ok(None);
    }

    let payload = &src[HEADER_LEN..HEADER_LEN + len];
    let item = match decode_from_slice::<T, _>(payload, config) {
        Ok((item, _)) => Ok(Some(item)),
        Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
    };

    src.advance(HEADER_LEN + len);
    item
}
