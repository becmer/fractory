// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

macro_rules! assert_no_err {
    ($expr:expr) => {{
        struct DebugHelper<T: std::fmt::Display>((usize, T));
        impl<T: std::fmt::Display> std::fmt::Debug for DebugHelper<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}: {}", self.0.0, self.0.1)
            }
        }
        let errors = $crate::test_utils::extract_errors($expr)
            .into_iter()
            .enumerate()
            .map(DebugHelper)
            .collect::<Vec<_>>();
        pretty_assertions::assert_matches!(errors.as_slice(), &[]);
    }};
}

macro_rules! assert_join_ok {
    ($expr:expr) => {{
        let result = futures::future::try_join_all($expr).await;
        pretty_assertions::assert_matches!(result, Ok(_));
        let Ok(result) = result else {
            unreachable!();
        };
        let result = result.into_iter().try_collect::<Vec<_>>();
        pretty_assertions::assert_matches!(result, Ok(_));
        let Ok(result) = result else {
            unreachable!();
        };
        result
    }};
}

macro_rules! flatten {
    ($expr:expr) => {
        ($expr).into_iter().flatten().collect::<Vec<_>>()
    };
}

use std::sync::Arc;

pub(crate) use assert_join_ok;
pub(crate) use assert_no_err;
pub(crate) use flatten;
use futures::future::join_all;
use tokio::{io, sync::Notify, task::JoinHandle};
use tokio_stream::StreamExt;

pub use self::{
    rec::record,
    time::{Sleep, Timeout, WithTimeout},
};
use crate::{
    DaemonClient, DaemonCx, DaemonError, DaemonListener, NativeDaemonConnector,
    NativeDaemonListener, NativeDaemonServer, msg::Response,
};

pub fn extract_errors<I, T, E>(items: I) -> Vec<E>
where
    I: IntoIterator<Item = Result<T, E>>,
{
    items.into_iter().filter_map(|r| r.err()).collect()
}

pub async fn spawn_listener(
    cx: DaemonCx,
    n: usize,
) -> JoinHandle<io::Result<Vec<NativeDaemonServer>>> {
    let notify = Arc::new(Notify::new());
    let handle = tokio::spawn({
        let notify = notify.clone();
        async move {
            let mut pipes = Vec::<NativeDaemonServer>::new();
            let mut listener = NativeDaemonListener::from_context(cx)?;
            notify.notify_one();

            for _ in 0..n {
                if let Some(server) = listener.next().await.transpose()? {
                    pipes.push(server);
                }
            }
            Ok(pipes)
        }
    });
    notify.notified().await;
    handle
}

type IoJoinHandle<T> = JoinHandle<io::Result<T>>;
type VecJoinHandle<T> = IoJoinHandle<Vec<T>>;

pub async fn spawn_echo_listener<T1, T2>(
    cx: DaemonCx,
    n: usize,
    listen_timeout: T1,
    respond_timeout: T2,
) -> VecJoinHandle<IoJoinHandle<NativeDaemonServer>>
where
    T1: Into<Timeout>,
    T2: Into<Timeout>,
{
    let listen_timeout = listen_timeout.into();
    let respond_timeout = respond_timeout.into();
    let notify = Arc::new(Notify::new());
    let handle = tokio::spawn({
        let notify = notify.clone();
        async move {
            let mut pipes = Vec::<IoJoinHandle<NativeDaemonServer>>::new();
            let mut listener = NativeDaemonListener::from_context(cx)?;
            notify.notify_one();

            for _ in 0..n {
                if let Some(mut server) = listen_timeout(listener.next()).await?.transpose()? {
                    pipes.push(tokio::spawn(
                        async move {
                            let request = server.recv().await?;
                            let response = Response::Echo(request);
                            server.send(response).await?;
                            Ok::<_, io::Error>(server)
                        }
                        .with_timeout_flattened(respond_timeout),
                    ));
                }
            }
            Ok(pipes)
        }
    });
    notify.notified().await;
    handle
}

pub async fn spawn_clients<S: Into<Sleep>>(
    cx: DaemonCx,
    n: usize,
    delay: S,
) -> Vec<Result<DaemonClient, DaemonError>> {
    let delay = delay.into();
    join_all((0..n).map(|_| async {
        delay.await;
        DaemonClient::connect_with(cx, NativeDaemonConnector).await
    }))
    .await
}

mod time {
    use std::{
        fmt,
        marker::PhantomData,
        pin::Pin,
        task::{Context, Poll},
        time::Duration,
    };

    use tokio::time::error::Elapsed;

    #[derive(Copy, Clone)]
    pub struct Sleep {
        duration: Duration,
    }
    impl From<u64> for Sleep {
        fn from(millis: u64) -> Self {
            let duration = Duration::from_millis(millis);
            Self { duration }
        }
    }
    impl From<Duration> for Sleep {
        fn from(duration: Duration) -> Self {
            Self { duration }
        }
    }
    impl IntoFuture for Sleep {
        type Output = ();
        type IntoFuture = tokio::time::Sleep;
        fn into_future(self) -> Self::IntoFuture {
            tokio::time::sleep(self.duration)
        }
    }
    impl fmt::Debug for Sleep {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Sleep").field(&self.duration).finish()
        }
    }

    #[derive(Copy, Clone)]
    pub struct Timeout {
        duration: Duration,
    }
    impl From<u64> for Timeout {
        fn from(millis: u64) -> Self {
            let duration = Duration::from_millis(millis);
            Self { duration }
        }
    }
    impl From<Duration> for Timeout {
        fn from(duration: Duration) -> Self {
            Self { duration }
        }
    }
    impl<T: IntoFuture> FnOnce<(T,)> for Timeout {
        type Output = tokio::time::Timeout<T::IntoFuture>;
        extern "rust-call" fn call_once(self, (future,): (T,)) -> Self::Output {
            tokio::time::timeout(self.duration, future)
        }
    }
    impl<T: IntoFuture> FnMut<(T,)> for Timeout {
        extern "rust-call" fn call_mut(&mut self, (future,): (T,)) -> Self::Output {
            tokio::time::timeout(self.duration, future)
        }
    }
    impl<T: IntoFuture> Fn<(T,)> for Timeout {
        extern "rust-call" fn call(&self, (future,): (T,)) -> Self::Output {
            tokio::time::timeout(self.duration, future)
        }
    }
    impl fmt::Debug for Timeout {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Timeout").field(&self.duration).finish()
        }
    }

    pub trait WithTimeout: IntoFuture + Sized {
        fn with_timeout<T: Into<Timeout>>(
            self,
            millis: T,
        ) -> tokio::time::Timeout<Self::IntoFuture> {
            let timeout = millis.into();
            timeout(self)
        }
        fn with_timeout_flattened<T: Into<Timeout>, U, E>(
            self,
            millis: T,
        ) -> FlattenedTimeout<Self::IntoFuture, E>
        where
            Self::IntoFuture: Future<Output = Result<U, E>>,
            E: From<Elapsed>,
        {
            let timeout = millis.into();
            FlattenedTimeout::new(timeout(self))
        }
    }
    impl<T: IntoFuture + Sized> WithTimeout for T {}

    pub struct FlattenedTimeout<F: Future, E> {
        inner: Pin<Box<tokio::time::Timeout<F>>>,
        _err: PhantomData<fn() -> E>,
    }
    impl<F: Future<Output = Result<T, E>>, T, E> FlattenedTimeout<F, E>
    where
        E: From<Elapsed>,
    {
        fn new(inner: tokio::time::Timeout<F>) -> Self {
            Self {
                inner: Box::pin(inner),
                _err: PhantomData,
            }
        }
    }
    impl<F: Future<Output = Result<T, E>>, T, E> Future for FlattenedTimeout<F, E>
    where
        E: From<Elapsed>,
    {
        type Output = F::Output;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            match self.inner.as_mut().poll(cx) {
                Poll::Ready(Ok(result)) => Poll::Ready(result),
                Poll::Ready(Err(e)) => Poll::Ready(Err(e.into())),
                Poll::Pending => Poll::Pending,
            }
        }
    }
}

mod rec {
    use std::{
        io,
        ops::{Deref, DerefMut},
        sync::Arc,
    };

    use memchr::memchr;
    use parking_lot::Mutex;
    use tracing::Dispatch;
    use tracing_subscriber::FmtSubscriber;

    pub fn record<F, T>(f: F) -> RecordedTrace<T>
    where
        F: FnOnce() -> T,
    {
        let buf = InnerBuffer::new();
        let dispatch: Dispatch = FmtSubscriber::builder()
            .with_env_filter("trace")
            .with_writer({
                let buf = buf.clone();
                move || buf.clone()
            })
            .with_level(true)
            .with_ansi(false)
            .into();
        let result = tracing::dispatcher::with_default(&dispatch, f);
        RecordedTrace {
            result,
            buf: buf.into_inner(),
        }
    }

    pub struct RecordedTrace<T> {
        result: T,
        buf: Vec<u8>,
    }
    #[allow(unused)]
    impl<T> RecordedTrace<T> {
        pub fn into_result(self) -> T {
            self.result
        }
        pub fn logged<S: AsRef<str>>(&self, s: S) -> bool {
            let s = s.as_ref().as_bytes();
            let (needle, rest) = match *s {
                [needle, ref rest @ ..] => (needle, rest),
                _ => return false,
            };

            let mut haystack = &*self.buf;
            while let Some(p) = memchr(needle, haystack) {
                haystack = &haystack[p + 1..];
                if rest.len() > haystack.len() {
                    break;
                }
                if &haystack[..rest.len()] == rest {
                    return true;
                }
            }
            false
        }
    }
    impl<T> Deref for RecordedTrace<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            &self.result
        }
    }
    impl<T> DerefMut for RecordedTrace<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.result
        }
    }

    #[derive(Clone)]
    struct InnerBuffer {
        shared: Arc<Mutex<Vec<u8>>>,
    }
    impl InnerBuffer {
        fn new() -> Self {
            Self {
                shared: Arc::new(Mutex::new(Vec::new())),
            }
        }
        fn into_inner(self) -> Vec<u8> {
            let mut shared = self.shared.lock();
            std::mem::take(&mut *shared)
        }
    }
    impl io::Write for InnerBuffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.shared.lock().write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
