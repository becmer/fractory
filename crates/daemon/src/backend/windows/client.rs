// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::time::Duration;

use futures::SinkExt;
use tokio::{
    io,
    net::windows::named_pipe::{ClientOptions, NamedPipeClient, PipeMode},
    time::sleep,
};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;
use windows::Win32::Foundation::ERROR_PIPE_BUSY;

use crate::{
    DaemonId,
    codec::ClientCodec,
    os_str_concat,
    msg::{Request, Response},
};

pub struct DaemonClient {
    id: DaemonId,
    framed: Framed<NamedPipeClient, ClientCodec>,
}

impl DaemonClient {
    pub fn id(&self) -> &DaemonId {
        &self.id
    }

    pub async fn connect_with_id<Q: Into<DaemonId>>(id: Q) -> io::Result<Self> {
        let id = id.into();
        let addr = os_str_concat!(r"\\.\pipe\", &id.name);

        for _ in 0..3 {
            let result = ClientOptions::new()
                .pipe_mode(PipeMode::Message)
                .open(&addr);
            match result {
                Ok(client) => {
                    let framed = Framed::new(client, ClientCodec::new());
                    return Ok(Self { id, framed });
                }
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => {
                    sleep(Duration::from_millis(10)).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("pipe {} is busy or unavailable", addr.to_string_lossy()),
        ))
    }

    pub async fn send(&mut self, request: Request) -> io::Result<Response> {
        self.framed.send(request).await?;
        self.framed.next().await.unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ))
        })
    }
}
