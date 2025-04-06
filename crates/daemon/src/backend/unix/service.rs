// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::ffi::OsString;

use anyhow::Result;

const SERVICE_NAME: &str = "fractory";

pub struct DaemonService {
    _todo: (),
}

impl DaemonService {
    pub fn start(args: Vec<OsString>) {
        if let Err(e) = Self::try_start(args) {
            log::error!("{SERVICE_NAME} failed: {e}");
        }
    }

    pub fn try_start(_: Vec<OsString>) -> Result<()> {
        todo!("unix support")
    }
}
