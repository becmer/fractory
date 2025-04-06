// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{ffi::OsString, fmt};

use tokio::{
    runtime::Runtime,
    task::JoinHandle,
    time::{Duration, sleep},
};
use tracing::{error, info};
pub use windows_service::define_windows_service;
use windows_service::{
    Result as ServiceResult,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler,
    service_control_handler::{ServiceControlHandlerResult, ServiceStatusHandle},
    service_dispatcher,
};

use crate::{DAEMON_NAME, DaemonCx};

type Result<T> = std::result::Result<T, DaemonServiceError>;
type SignalTx = flume::Sender<()>;
type SignalRx = flume::Receiver<()>;

pub fn dispatch(service_main: extern "system" fn(u32, *mut *mut u16)) -> Result<()> {
    service_dispatcher::start(DAEMON_NAME, service_main)?;
    Ok(())
}

/// A daemon service.
///
/// # Ideas
/// - Add a timeout on `initialized_rx` to avoid indefinite hangs.
/// - Handle `Pause`/`Continue` if you ever add background tasks.
/// - Add telemetry to distinguish graceful, forced, and idle shutdowns.
/// - Track uptime for metrics (could log duration on `Stopped`).
pub struct DaemonService {
    handle: ServiceStatusHandle,
}

impl DaemonService {
    pub fn start(args: Vec<OsString>) {
        if let Err(error) = Self::try_start(args) {
            error!(%error, "{DAEMON_NAME} failed");
        }
    }

    pub fn try_start(_: Vec<OsString>) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = flume::bounded::<()>(1);
        Self::register(shutdown_tx)?.run_inner(shutdown_rx)
    }

    async fn run_daemon(shutdown_rx: SignalRx, initialized_tx: SignalTx) -> Result<()> {
        // TODO: In reality, we would like to cede this logic into a common implementation.
        let running = tokio::select! {
            _ = shutdown_rx.recv_async() => {
                info!("service shutdown requested");
                false
            }
            _ = sleep(Duration::from_secs(1)) => {
                info!("daemon initialized");
                let _ = initialized_tx.send(());
                true
            }
        };
        if running {
            tokio::select! {
                _ = shutdown_rx.recv_async() => {
                    info!("service shutdown requested");
                }
                _ = sleep(Duration::from_secs(5)) => {
                    info!("daemon finished work");
                }
            }
        }
        Ok(())
    }

    fn run_inner(self, shutdown_rx: SignalRx) -> Result<()> {
        let (initialized_tx, initialized_rx) = flume::bounded::<()>(1);
        self.set_status(ServiceState::StartPending)?;

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        self.spawn_on(&rt, |this| async move {
            initialized_rx.recv_async().await?;
            this.set_status(ServiceState::Running)?;
            Ok::<_, DaemonServiceError>(())
        });
        let result = rt.block_on(Self::run_daemon(shutdown_rx, initialized_tx));

        match (self.set_status(ServiceState::Stopped), result) {
            (Ok(_), result) => result,
            (_, Err(e)) => Err(e),
            (Err(e), _) => Err(e.into()),
        }
    }

    fn register(shutdown_tx: flume::Sender<()>) -> ServiceResult<Self> {
        let handle = service_control_handler::register(DAEMON_NAME, move |event| match event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop | ServiceControl::Preshutdown | ServiceControl::Shutdown => {
                info!("stop signal from SCM received");
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        })?;
        Ok(Self { handle })
    }

    fn spawn_on<F, Fut>(&self, rt: &Runtime, f: F) -> JoinHandle<<Fut as Future>::Output>
    where
        F: FnOnce(Self) -> Fut,
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        rt.spawn(f(Self {
            handle: self.handle,
        }))
    }

    fn set_status(&self, target_state: ServiceState) -> ServiceResult<()> {
        info!(?target_state, "setting new service status");
        let controls_accepted = match target_state {
            ServiceState::Running | ServiceState::StartPending => {
                ServiceControlAccept::STOP
                    | ServiceControlAccept::PRESHUTDOWN
                    | ServiceControlAccept::SHUTDOWN
            }
            ServiceState::Stopped => ServiceControlAccept::empty(),
            _ => unimplemented!(),
        };
        let wait_hint = match target_state {
            ServiceState::StartPending => Duration::from_secs(10),
            _ => Duration::from_secs(0),
        };
        self.handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: target_state,
            controls_accepted,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint,
            process_id: None,
        })
    }
}

#[derive(Debug)]
pub enum DaemonServiceError {
    Io(std::io::Error),
    Scm(windows_service::Error),
    Rx(flume::RecvError),
}

impl From<std::io::Error> for DaemonServiceError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<windows_service::Error> for DaemonServiceError {
    fn from(e: windows_service::Error) -> Self {
        Self::Scm(e)
    }
}

impl From<flume::RecvError> for DaemonServiceError {
    fn from(e: flume::RecvError) -> Self {
        Self::Rx(e)
    }
}

impl fmt::Display for DaemonServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Io(ref err) => fmt::Display::fmt(err, f),
            Self::Scm(ref err) => fmt::Display::fmt(err, f),
            Self::Rx(ref err) => fmt::Display::fmt(err, f),
        }
    }
}

impl std::error::Error for DaemonServiceError {}
