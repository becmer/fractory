// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

mod client;
mod lock;
mod server;
#[cfg(feature = "service")]
pub mod service;

pub use self::{
    client::DaemonClient,
    lock::DaemonLock,
    server::{DaemonListener, DaemonServer},
};
