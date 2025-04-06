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

use std::{sync::Arc, time::Duration};

pub(crate) use assert_join_ok;
pub(crate) use assert_no_err;
pub(crate) use flatten;
use futures::future::join_all;
use tokio::{io, sync::Notify, task::JoinHandle, time::sleep};
use tokio_stream::StreamExt;

use crate::{DaemonClient, DaemonId, DaemonListener, DaemonServer, msg::Response};

pub fn millis(n: u64) -> Duration {
    Duration::from_millis(n)
}

pub fn extract_errors<I, T, E>(items: I) -> Vec<E>
where
    I: IntoIterator<Item = Result<T, E>>,
{
    items.into_iter().filter_map(|r| r.err()).collect()
}

#[cfg(windows)]
pub async fn spawn_listener(id: &DaemonId, n: usize) -> JoinHandle<io::Result<Vec<DaemonServer>>> {
    let id = id.clone();
    let notify = Arc::new(Notify::new());
    let handle = tokio::spawn({
        let notify = notify.clone();
        async move {
            let mut pipes = Vec::<DaemonServer>::new();
            let mut listener = DaemonListener::with_id(id).await;
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

#[cfg(windows)]
pub async fn spawn_echo_listener(
    id: &DaemonId,
    n: usize,
) -> VecJoinHandle<IoJoinHandle<DaemonServer>> {
    let id = id.clone();
    let notify = Arc::new(Notify::new());
    let handle = tokio::spawn({
        let notify = notify.clone();
        async move {
            let mut pipes = Vec::<IoJoinHandle<DaemonServer>>::new();
            let mut listener = DaemonListener::with_id(id).await;
            notify.notify_one();

            for _ in 0..n {
                if let Some(mut server) = listener.next().await.transpose()? {
                    pipes.push(tokio::spawn(async move {
                        let request = server.recv().await?;
                        let response = Response::Echo(request);
                        server.send(response).await?;
                        Ok::<_, io::Error>(server)
                    }));
                }
            }
            Ok(pipes)
        }
    });
    notify.notified().await;
    handle
}

#[cfg(windows)]
pub async fn spawn_clients(
    id: &DaemonId,
    n: usize,
    delay: Option<Duration>,
) -> Vec<io::Result<DaemonClient>> {
    join_all((0..n).map(|_| async {
        if let Some(delay) = delay {
            sleep(delay).await;
        }
        DaemonClient::connect_with_id(id).await
    }))
    .await
}
