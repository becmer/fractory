// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use futures::SinkExt;
use tokio::{
    io,
    io::{AsyncRead, AsyncWrite},
};
use tokio_stream::{Stream, StreamExt};
use tokio_util::codec::Framed;

#[cfg(all(unix, feature = "service"))]
pub use self::unix::service;
#[cfg(unix)]
pub use self::unix::{DaemonCxAttachment, DaemonLock, NativeDaemonConnector, NativeDaemonListener};
#[cfg(all(windows, feature = "service"))]
pub use self::windows::service;
#[cfg(windows)]
pub use self::windows::{
    DaemonCxAttachment, DaemonLock, NativeDaemonConnector, NativeDaemonListener,
};
use crate::{
    DaemonCx, DaemonError,
    codec::{ClientCodec, ServerCodec},
    cx::DaemonCxPayload,
    msg::{Request, Response},
};

#[cfg(not(any(windows, unix)))]
compile_error!("daemon is not supported on this platform");

impl DaemonLock {
    pub fn system() -> Result<Self, DaemonError> {
        Self::try_acquire_lock(DaemonCxPayload::system())
    }

    pub fn user() -> Result<Self, DaemonError> {
        Self::try_acquire_lock(DaemonCxPayload::user())
    }
}

pub trait DaemonListener: Stream<Item = io::Result<DaemonServer<Self::Stream>>> + Sized {
    type Stream: AsyncRead + AsyncWrite + Unpin;
    fn new() -> io::Result<Self> {
        Self::from_context(DaemonCx::get())
    }
    fn from_context(cx: DaemonCx) -> io::Result<Self>;
}

pub type NativeDaemonServer = DaemonServer<<NativeDaemonListener as DaemonListener>::Stream>;

#[derive(Debug)]
pub struct DaemonServer<T> {
    cx: DaemonCx,
    framed: Framed<T, ServerCodec>,
}
impl<T: AsyncRead + AsyncWrite + Unpin> DaemonServer<T> {
    pub fn new(cx: DaemonCx, inner: T) -> Self {
        Self {
            cx,
            framed: Framed::new(inner, ServerCodec::new()),
        }
    }
    pub fn context(&self) -> DaemonCx {
        self.cx
    }
    pub async fn recv(&mut self) -> io::Result<Request> {
        self.framed.next().await.unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ))
        })
    }
    pub async fn send(&mut self, response: Response) -> io::Result<()> {
        self.framed.send(response).await
    }
}

pub struct DaemonClient<C: DaemonConnector = NativeDaemonConnector> {
    cx: DaemonCx,
    framed: Framed<C::Stream, ClientCodec>,
}
impl<C: DaemonConnector + Default> DaemonClient<C> {
    pub async fn connect() -> Result<Self, DaemonError> {
        let cx = DaemonCx::get();
        let connector = C::default();
        Self::connect_with(cx, connector).await
    }
}
impl<C: DaemonConnector> DaemonClient<C> {
    pub async fn connect_with(cx: DaemonCx, mut connector: C) -> Result<Self, DaemonError> {
        let stream = connector.connect(cx).await?;
        let framed = Framed::new(stream, ClientCodec::new());
        Ok(Self { cx, framed })
    }
    pub fn context(&self) -> DaemonCx {
        self.cx
    }
    pub async fn send(&mut self, request: Request) -> io::Result<Response> {
        self.framed.send(request).await?;
        self.framed.next().await.unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ))
        })
    }
}

pub trait DaemonConnector {
    type Stream: AsyncRead + AsyncWrite + Unpin;
    type Future: Future<Output = Result<Self::Stream, DaemonError>>;
    fn connect(&mut self, cx: DaemonCx) -> Self::Future;
}
impl<T, F, Fut> DaemonConnector for F
where
    T: AsyncRead + AsyncWrite + Unpin,
    F: FnMut(DaemonCx) -> Fut,
    Fut: Future<Output = Result<T, DaemonError>>,
{
    type Stream = T;
    type Future = Fut;
    fn connect(&mut self, cx: DaemonCx) -> Self::Future {
        self(cx)
    }
}

#[cfg(test)]
mod tests {
    use ntest::timeout;
    use pretty_assertions::{assert_eq, assert_matches};
    use tracing::instrument;
    use tracing_subscriber::{EnvFilter, fmt::format};

    use crate::{
        DaemonCx,
        msg::{Request, Response},
        test_utils::*,
    };

    fn init() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::from_default_env().add_directive("fractory=trace".parse().unwrap()),
            )
            .event_format(format().compact())
            .with_test_writer()
            .try_init();
    }

    #[instrument(level = "trace")]
    #[tokio::test]
    #[timeout(500)]
    async fn connect_multiple_clients() {
        init();
        const CLIENTS: usize = 10;

        let cx = DaemonCx::random_in_user_scope();
        let listener = spawn_listener(cx, CLIENTS).await;
        let clients = spawn_clients(cx, CLIENTS, 0).await;

        assert_no_err!(clients);
        assert_join_ok!(Some(listener));
    }

    #[instrument(level = "trace")]
    #[tokio::test]
    #[timeout(500)]
    async fn server_echoes_message() {
        init();
        let cx = DaemonCx::random_in_user_scope();
        let listener = spawn_echo_listener(cx, 1, 500, 500).await;
        let mut client = spawn_clients(cx, 1, 0)
            .await
            .remove(0)
            .expect("client connected");

        let request = Request {
            kind: 42,
            data: "ping".to_owned(),
        };
        let response = client.send(request.clone()).with_timeout(500).await;

        assert_matches!(response, Ok(Ok(Response::Echo(_))));
        let Ok(Ok(Response::Echo(response))) = response else {
            unreachable!();
        };
        assert_eq!(request, response);
        let listener = assert_join_ok!(Some(listener));
        let _ = assert_join_ok!(flatten!(listener));
    }
}
