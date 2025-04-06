// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

mod pidfile;
#[cfg(feature = "service")]
pub mod service;

use std::{
    borrow::Cow,
    fmt,
    mem::ManuallyDrop,
    ops::Deref,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    pin::Pin,
    sync::atomic::{
        AtomicBool,
        Ordering::{Relaxed, SeqCst},
    },
    task::{
        Context, Poll,
        Poll::{Pending, Ready},
    },
};

use futures::Stream;
use tokio::{
    io,
    net::{UnixListener, UnixStream},
};
use tokio_util::codec::Framed;
use tracing::{debug, instrument, trace, warn};

use self::pidfile::Pidfile;
use crate::{
    DaemonCx, DaemonCxPayload, DaemonError, DaemonId, DaemonScope, DaemonServer,
    codec::ServerCodec, os_str_concat,
};

#[derive(Clone)]
pub struct DaemonCxAttachment {
    pub runtime_dir: RuntimeDirectory,
    pub pidfile_path: PathBuf,
    pub socket_path: PathBuf,
    _ctor: (),
}

impl DaemonCxAttachment {
    #[instrument(level = "trace", skip(id))]
    pub(crate) fn new(id: &DaemonId) -> Result<Self, DaemonError> {
        let runtime_dir = RuntimeDirectory::new(id);
        if runtime_dir.temp {
            warn!(
                rundir = %runtime_dir.as_path().display(),
                "Using temporary runtime directory. This may break socket-based coordination.",
            );
            warn!("Set XDG_RUNTIME_DIR or ensure a proper runtime dir is configured.")
        }

        std::fs::create_dir_all(&runtime_dir)
            .and_then(|_| {
                std::fs::set_permissions(&runtime_dir, std::fs::Permissions::from_mode(0o700))
            })
            .map_err(|e| DaemonError::RundirUnavailable(id.clone(), e))?;
        trace!(rundir = %runtime_dir.as_path().display(), "rundir available");

        let pidfile_path = runtime_dir.join(&id.name).with_extension("pid");
        let socket_path = runtime_dir.join(&id.name).with_extension("sock");

        Ok(Self {
            runtime_dir,
            pidfile_path,
            socket_path,
            _ctor: (),
        })
    }
}
impl fmt::Debug for DaemonCxAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaemonCxAttachment")
            .field("runtime_dir", &self.runtime_dir)
            .field("pidfile_path", &self.pidfile_path)
            .field("socket_path", &self.socket_path)
            .finish()
    }
}

pub struct RuntimeDirectory {
    path: PathBuf,
    temp: bool,
    owned: AtomicBool,
}
impl RuntimeDirectory {
    #[instrument(level = "trace", skip(id))]
    fn new(id: &DaemonId) -> Self {
        let path = match id.scope {
            DaemonScope::System => find_runtime_path(true),
            DaemonScope::User => match std::env::var_os("XDG_RUNTIME_DIR") {
                Some(p) if Path::new(&p).is_dir() => Some(Cow::Owned(PathBuf::from(p))),
                _ => find_runtime_path(false),
            },
        };
        let (path, temp) = match path {
            Some(path) => (path.join(&id.name), false),
            None => {
                let name = match id.scope {
                    DaemonScope::System => Cow::Borrowed(Path::new(&id.name)),
                    DaemonScope::User => {
                        let uid = nix::unistd::Uid::current().to_string();
                        Cow::Owned(PathBuf::from(os_str_concat!(&id.name, "-", uid)))
                    }
                };
                (std::env::temp_dir().join(name), true)
            }
        };
        Self {
            path,
            temp,
            owned: AtomicBool::new(false),
        }
    }
    fn mark_as_owned(&self) {
        self.owned.store(true, SeqCst);
    }
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}
fn find_runtime_path(system: bool) -> Option<Cow<'static, Path>> {
    let path = ["/run", "/var/run"]
        .into_iter()
        .map(Path::new)
        .map(Cow::Borrowed)
        .find(|p| p.is_dir())?;
    if system {
        return Some(path);
    }
    Some(Cow::Owned(
        path.join(format!("user/{}", nix::unistd::Uid::current())),
    ))
}
impl AsRef<Path> for RuntimeDirectory {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}
impl Deref for RuntimeDirectory {
    type Target = Path;
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}
impl Clone for RuntimeDirectory {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            temp: self.temp,
            owned: AtomicBool::new(false),
        }
    }
}
impl Drop for RuntimeDirectory {
    fn drop(&mut self) {
        if self.temp && self.owned.load(Relaxed) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
impl fmt::Debug for RuntimeDirectory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeDirectory")
            .field("path", &self.path)
            .field("temp", &self.temp)
            .field("owned", &self.owned.load(Relaxed))
            .finish()
    }
}

pub struct DaemonLock {
    cx: DaemonCx,
    flock: ManuallyDrop<Pidfile>,
}
impl DaemonLock {
    #[instrument(level = "debug", skip(cx))]
    pub fn try_acquire_lock(cx: DaemonCxPayload) -> Result<Self, DaemonError> {
        if nix::unistd::Uid::effective().is_root() {
            warn!("Running fractory as root is not recommended. Use a dedicated user instead.");
        }

        trace!(%cx.id, "attempting to acquire daemon lock");

        let path = &cx.pidfile_path;
        trace!(?path, "using flock path");

        let flock = ManuallyDrop::new(
            Pidfile::open(path).map_err(|e| DaemonError::LockFailed(cx.id.clone(), Box::new(e)))?,
        );

        let cx = cx.install();
        cx.runtime_dir.mark_as_owned();
        debug!(%cx.id, "daemon lock acquired successfully");
        Ok(Self { cx, flock })
    }
}
impl Drop for DaemonLock {
    fn drop(&mut self) {
        trace!(%self.cx.id, "going to release daemon lock");
        unsafe { ManuallyDrop::drop(&mut self.flock) };
        debug!(%self.cx.id, "daemon lock released successfully");
    }
}

pub struct NativeDaemonListener {
    cx: DaemonCx,
    inner: UnixListener,
}
impl super::DaemonListener for NativeDaemonListener {
    type Stream = UnixStream;
    #[instrument(level = "trace", skip(cx))]
    fn from_context(cx: DaemonCx) -> io::Result<Self> {
        trace!("going to bind: {}", cx.socket_path.display());
        match UnixListener::bind(&cx.socket_path) {
            Ok(inner) => {
                trace!("socket bound: {}", cx.socket_path.display());
                Ok(Self { cx, inner })
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }
}
impl Stream for NativeDaemonListener {
    type Item = io::Result<DaemonServer<UnixStream>>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.poll_accept(cx) {
            Ready(Ok((stream, _))) => Ready(Some(Ok(DaemonServer {
                cx: self.cx,
                framed: Framed::new(stream, ServerCodec::new()),
            }))),
            Ready(Err(e)) => Ready(Some(Err(e))),
            Pending => Pending,
        }
    }
}

#[derive(Default)]
pub struct NativeDaemonConnector;
impl super::DaemonConnector for NativeDaemonConnector {
    type Stream = UnixStream;
    type Future = impl Future<Output = Result<Self::Stream, DaemonError>>;
    fn connect(&mut self, cx: DaemonCx) -> Self::Future {
        async move {
            UnixStream::connect(&cx.socket_path)
                .await
                .map_err(|e| DaemonError::ConnectFailed(cx, e))
        }
    }
}
