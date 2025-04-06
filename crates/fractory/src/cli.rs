// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{ffi::OsString, path::Path};

use anyhow::Result;
use clap::{Parser, Subcommand};
use fractory::FractoryCx;
use fractory_daemon::service;

#[cfg(windows)]
service::define_windows_service!(ffi_service_main, service_main);

#[cfg(windows)]
fn service_main(args: Vec<OsString>) {
    service::DaemonService::start(args);
}

#[derive(Parser)]
#[command(
    name = "fractory",
    version,
    about = "Nonlinear intent flow tracking for mosaic thinkers",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Maintain the fractory service
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
    /// Initialize the fractory metadata directory
    Init,
}

#[derive(Subcommand)]
enum ServiceCommand {
    Start,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Service { command } => match command {
            #[cfg(windows)]
            ServiceCommand::Start => service::dispatch(ffi_service_main)?,
            #[cfg(unix)]
            ServiceCommand::Start => todo!("unix support"),
            #[cfg(not(any(windows, unix)))]
            ServiceCommand::Start => compile_error!("daemon is not supported on this platform"),
        },
        Command::Init => {
            let _ = FractoryCx::init(Path::new(".")).await?;
        }
    }
    Ok(())
}
