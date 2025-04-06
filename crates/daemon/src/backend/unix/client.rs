// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use tokio::io;

use crate::{
    DaemonId,
    msg::{Request, Response},
};

pub struct DaemonClient {
    id: DaemonId,
}

impl DaemonClient {
    pub fn id(&self) -> &DaemonId {
        &self.id
    }

    pub async fn connect_with_id<Q: Into<DaemonId>>(_id: Q) -> io::Result<Self> {
        todo!("unix support")
    }

    pub async fn send(&mut self, _request: Request) -> io::Result<Response> {
        todo!("unix support")
    }
}
