// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{process::ExitCode, thread, time::Duration};

use fractory_daemon::{DaemonCxPayload, DaemonLock, DaemonScope};
use tracing::error;
use tracing_subscriber::fmt::format;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .event_format(format().compact())
        .init();

    let name = std::env::args_os().nth(1).expect("missing lock name");
    let cx =
        DaemonCxPayload::new(DaemonScope::User, name).expect("failed to create daemon context");
    match DaemonLock::try_acquire_lock(cx) {
        Ok(lock) => {
            thread::sleep(Duration::from_secs(30));
            let _ = lock;
            ExitCode::SUCCESS
        }
        Err(error) => {
            error!(%error, "failed to acquire lock");
            ExitCode::FAILURE
        }
    }
}
