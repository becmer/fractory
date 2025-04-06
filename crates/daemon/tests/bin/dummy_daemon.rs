// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{io::Write, process::ExitCode, thread, time::Duration};

use env_logger::{Builder, Env};
use fractory_daemon::{DaemonLock, DaemonScope};

fn main() -> ExitCode {
    let pid = std::process::id();
    Builder::from_env(Env::default().default_filter_or("info"))
        .format(move |f, record| {
            let ts = f.timestamp_millis(); // includes milliseconds
            writeln!(f, "[{} {}] {}", ts, pid, record.args())
        })
        .init();

    let name = std::env::args_os().nth(1).expect("missing lock name");
    match DaemonLock::try_acquire_lock(name, DaemonScope::User) {
        Ok(lock) => {
            thread::sleep(Duration::from_secs(30));
            let _ = lock;
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error!("{e}");
            ExitCode::FAILURE
        }
    }
}
