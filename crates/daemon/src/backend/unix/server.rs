// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io;
use tokio_stream::Stream;

use crate::{
    DaemonId,
    msg::{Request, Response},
};

pub struct DaemonListener {
    _todo: (),
}

impl DaemonListener {
    pub async fn new() -> Self {
        Self::with_id(DaemonId::get()).await
    }

    pub(crate) async fn with_id<Q: Into<DaemonId>>(id: Q) -> Self {
        todo!("unix support")
    }
}

impl Stream for DaemonListener {
    type Item = io::Result<DaemonServer>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        todo!("unix support")
    }
}

pub struct DaemonServer {
    id: DaemonId,
}

impl DaemonServer {
    pub fn id(&self) -> &DaemonId {
        &self.id
    }

    pub async fn recv(&mut self) -> io::Result<Request> {
        todo!("unix support")
    }

    pub async fn send(&mut self, response: Response) -> io::Result<()> {
        todo!("unix support")
    }
}
