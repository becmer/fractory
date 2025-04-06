// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer
// TODO: Implement `FactoryCx` after daemon functionality hardened.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use anyhow::Result;
use gix::ThreadSafeRepository;

#[derive(Clone, Debug)]
pub struct FractoryCx {
    path: PathBuf,
    repo: ThreadSafeRepository,
}

impl FractoryCx {
    pub async fn open<P: AsRef<Path>>(_path: P) -> Result<FractoryCx> {
        todo!("repo context")
    }

    pub async fn init<P: AsRef<Path>>(_path: P) -> Result<Self> {
        todo!("repo context")
    }

    pub fn start_daemon(self) {
        todo!("repo context")
    }
}
