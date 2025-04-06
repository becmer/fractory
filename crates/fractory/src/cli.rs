// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::path::Path;

use anyhow::Result;
use clap::{Parser, Subcommand};
use fractory::FractoryCx;
use fractory_daemon::{DaemonCxPayload, service};

#[cfg(windows)]
service::define_windows_service!(ffi_service_main, service_main);

#[cfg(windows)]
fn service_main(args: Vec<std::ffi::OsString>) {
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

impl Command {
    async fn dipatch(self) -> Result<()> {
        match self {
            Command::Service { command } => command.dipatch().await?,
            Command::Init => {
                let _ = FractoryCx::init(Path::new(".")).await?;
            }
        }
        Ok(())
    }
}

#[derive(Subcommand)]
enum ServiceCommand {
    #[cfg(unix)]
    Generate {
        #[arg(long)]
        system: bool,
    },
    Start,
}

impl ServiceCommand {
    #[cfg(windows)]
    async fn dipatch(self) -> Result<()> {
        match self {
            ServiceCommand::Start => service::dispatch(ffi_service_main)?,
        }
        Ok(())
    }

    #[cfg(unix)]
    async fn dipatch(self) -> Result<()> {
        use ServiceCommand::*;
        match self {
            Generate { system } => {
                let cx = if system {
                    DaemonCxPayload::system()
                } else {
                    DaemonCxPayload::user()
                };
                println!("{}", service::ServiceUnit::from_context(cx));
            }
            Start => todo!("unix support"),
        }
        Ok(())
    }

    #[cfg(not(any(windows, unix)))]
    async fn dipatch(self) -> Result<()> {
        compile_error!("daemon is not supported on this platform");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.command.dipatch().await
}
