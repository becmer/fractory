// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

mod fqueue;
#[cfg(feature = "service")]
pub mod service;

use std::{
    ffi::{OsStr, OsString},
    fmt,
    ops::Deref,
    pin::Pin,
    task::{
        Context, Poll,
        Poll::{Pending, Ready},
    },
    time::Duration,
};

use futures::{Stream, future::BoxFuture};
use tokio::{
    io,
    net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, PipeMode, ServerOptions,
    },
    time::sleep,
};
use tracing::{debug, error, instrument, trace};
use widestring::U16CString;
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
        System::Threading::CreateMutexW,
    },
    core::PCWSTR,
};

use crate::{
    DaemonCx, DaemonCxPayload, DaemonError, DaemonId, DaemonScope, DaemonServer,
    backend::windows::fqueue::{Factory, FactoryQueue},
    os_str_concat,
};

type BoxListener = BoxFuture<'static, io::Result<NamedPipeServer>>;

const ERROR_PIPE_BUSY: i32 = windows::Win32::Foundation::ERROR_PIPE_BUSY.0 as i32;
const FACTORY_QUEUE_CAPACITY: usize = 4;

#[derive(Clone)]
pub struct DaemonCxAttachment {
    pub mutex_name: U16CString,
    pub pipe_addr: OsString,
    _ctor: (),
}
impl DaemonCxAttachment {
    pub(crate) fn new(id: &DaemonId) -> Result<Self, DaemonError> {
        let scope = match id.scope {
            DaemonScope::System => OsStr::new(r"Global\"),
            DaemonScope::User => OsStr::new(r"Local\"),
        };
        let Ok(mutex_name) = U16CString::from_os_str(os_str_concat!(scope, &id.name)) else {
            return Err(id.into_invalid_name_error());
        };

        let pipe_addr = os_str_concat!(r"\\.\pipe\", &id.name);

        Ok(Self {
            mutex_name,
            pipe_addr,
            _ctor: (),
        })
    }
}
impl fmt::Debug for DaemonCxAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaemonCxAttachment").finish()
    }
}

#[derive(Debug)]
pub struct DaemonLock {
    cx: DaemonCx,
    handle: HANDLE,
}
impl DaemonLock {
    #[instrument(level = "debug", skip(cx))]
    pub fn try_acquire_lock(cx: DaemonCxPayload) -> Result<Self, DaemonError> {
        trace!(%cx.id, "attempting to acquire daemon lock");

        let name = PCWSTR(cx.mutex_name.as_ptr());
        trace!(mutex = %cx.mutex_name.display(), "using mutex name");

        let handle = unsafe {
            CreateMutexW(None, true, name)
                .map_err(|e| DaemonError::LockFailed(cx.id.clone(), e.into()))?
        };
        if ERROR_ALREADY_EXISTS == unsafe { GetLastError() } {
            error!(%cx.id, "daemon already running");
            return Err(cx.into_already_running_error());
        }

        let cx = cx.install();
        debug!(%cx.id, "daemon lock acquired successfully");
        Ok(Self { cx, handle })
    }
}
impl Drop for DaemonLock {
    fn drop(&mut self) {
        trace!(%self.cx.id, "going to release daemon lock");
        let _ = unsafe { CloseHandle(self.handle) };
        debug!(%self.cx.id, "daemon lock released successfully");
    }
}

pub struct NativeDaemonListener {
    factory: FactoryQueue<NamedPipeFactory>,
    listener: BoxListener,
}
impl super::DaemonListener for NativeDaemonListener {
    type Stream = NamedPipeServer;
    #[instrument(level = "trace", skip(cx))]
    fn from_context(cx: DaemonCx) -> io::Result<Self> {
        trace!(%cx.id, addr = %cx.pipe_addr.display(), "binding named pipe listener");
        let mut factory = FactoryQueue::<NamedPipeFactory>::from(cx, FACTORY_QUEUE_CAPACITY);
        let listener = factory.next();

        Ok(Self { factory, listener })
    }
}
impl Stream for NativeDaemonListener {
    type Item = io::Result<DaemonServer<NamedPipeServer>>;
    #[instrument(level = "trace", skip(self, cx))]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.listener.as_mut().poll(cx) {
            Ready(result) => {
                self.listener = self.factory.next();
                Ready(Some(result.map(|s| DaemonServer::new(self.factory.cx, s))))
            }
            Pending => Pending,
        }
    }
}

#[derive(Default)]
pub struct NativeDaemonConnector;
impl super::DaemonConnector for NativeDaemonConnector {
    type Stream = NamedPipeClient;
    type Future = impl Future<Output = io::Result<Self::Stream>>;
    fn connect(&mut self, cx: DaemonCx) -> Self::Future {
        async move {
            for attempt in 0..3 {
                let result = ClientOptions::new()
                    .pipe_mode(PipeMode::Message)
                    .open(&cx.pipe_addr);
                match result {
                    Ok(client) => {
                        trace!(%cx.id, "connected successfully");
                        return Ok(client)
                    },
                    Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                        trace!(attempt, "pipe busy, retrying");
                        sleep(Duration::from_millis(10)).await;
                    }
                    Err(e) => return Err(e),
                }
            }

            Err(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                format!("{} is busy or unavailable", cx.pipe_addr.display()),
            ))
        }
    }
}

struct NamedPipeFactory {
    cx: DaemonCx,
}
impl From<DaemonCx> for NamedPipeFactory {
    fn from(cx: DaemonCx) -> Self {
        Self { cx }
    }
}
impl Factory for NamedPipeFactory {
    type Seed = io::Result<NamedPipeServer>;
    type Item = BoxListener;
    fn create(&mut self) -> Self::Seed {
        ServerOptions::new()
            .first_pipe_instance(false)
            .pipe_mode(PipeMode::Message)
            .access_inbound(true)
            .access_outbound(true)
            .reject_remote_clients(true)
            .create(&self.cx.pipe_addr)
    }
    fn settle(&mut self, pipe: Self::Seed) -> Self::Item {
        Box::pin(async move {
            let pipe = pipe?;
            pipe.connect().await?;
            Ok::<_, io::Error>(pipe)
        })
    }
}
impl Deref for NamedPipeFactory {
    type Target = Self;
    fn deref(&self) -> &Self::Target {
        self
    }
}
