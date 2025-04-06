// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{
    ffi::OsString,
    ops::Deref,
    pin::Pin,
    task::{
        Context, Poll,
        Poll::{Pending, Ready},
    },
};

use futures::{SinkExt, future::BoxFuture};
use tokio::{
    io,
    net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions},
};
use tokio_stream::{Stream, StreamExt};
use tokio_util::codec::Framed;

use crate::{
    DaemonId,
    backend::windows::fqueue::{Factory, FactoryQueue},
    codec::ServerCodec,
    msg::{Request, Response},
    os_str_concat,
};

const DEFAULT_CAPACITY: usize = 4;

type BoxListener = BoxFuture<'static, io::Result<NamedPipeServer>>;

struct NamedPipeFactory {
    id: DaemonId,
    addr: OsString,
}

impl From<DaemonId> for NamedPipeFactory {
    fn from(id: DaemonId) -> Self {
        let addr = os_str_concat!(r"\\.\pipe\", &id.name);
        Self { id, addr }
    }
}

impl Factory for NamedPipeFactory {
    type Seed = io::Result<NamedPipeServer>;
    type Item = BoxListener;

    fn create(&mut self) -> Self::Seed {
        ServerOptions::new()
            .first_pipe_instance(false)
            .pipe_mode(PipeMode::Message)
            .access_inbound(true)
            .access_outbound(true)
            .reject_remote_clients(true)
            .create(&self.addr)
    }

    fn settle(&mut self, pipe: Self::Seed) -> Self::Item {
        Box::pin(async move {
            let pipe = pipe?;
            pipe.connect().await?;
            Ok::<_, io::Error>(pipe)
        })
    }
}

impl Deref for NamedPipeFactory {
    type Target = DaemonId;
    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

pub struct DaemonListener {
    factory: FactoryQueue<NamedPipeFactory>,
    listener: BoxListener,
}

impl DaemonListener {
    pub async fn new() -> Self {
        Self::with_id(DaemonId::get()).await
    }

    pub(crate) async fn with_id<Q: Into<DaemonId>>(id: Q) -> Self {
        let mut factory = FactoryQueue::<NamedPipeFactory>::from(id.into(), DEFAULT_CAPACITY);
        let listener = factory.next();

        Self { factory, listener }
    }
}

impl Stream for DaemonListener {
    type Item = io::Result<DaemonServer>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.listener.as_mut().poll(cx) {
            Ready(result) => {
                self.listener = self.factory.next();
                Ready(Some(result.map(|server| {
                    let framed = Framed::new(server, ServerCodec::new());
                    DaemonServer {
                        id: self.factory.clone(),
                        framed,
                    }
                })))
            }
            Pending => Pending,
        }
    }
}

#[derive(Debug)]
pub struct DaemonServer {
    id: DaemonId,
    framed: Framed<NamedPipeServer, ServerCodec>,
}

impl DaemonServer {
    pub fn id(&self) -> &DaemonId {
        &self.id
    }

    pub async fn recv(&mut self) -> io::Result<Request> {
        self.framed.next().await.unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ))
        })
    }

    pub async fn send(&mut self, response: Response) -> io::Result<()> {
        self.framed.send(response).await
    }
}
