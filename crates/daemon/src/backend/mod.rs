// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(all(unix, feature = "service"))]
pub use self::unix::service;
#[cfg(unix)]
pub use self::unix::{DaemonClient, DaemonListener, DaemonLock, DaemonServer};
#[cfg(all(windows, feature = "service"))]
pub use self::windows::service;
#[cfg(windows)]
pub use self::windows::{DaemonClient, DaemonListener, DaemonLock, DaemonServer};
use crate::{DEFAULT_DAEMON_NAME, DaemonError, DaemonScope};

#[cfg(not(any(windows, unix)))]
compile_error!("daemon is not supported on this platform");

impl DaemonLock {
    pub fn system() -> Result<Self, DaemonError> {
        Self::try_acquire_lock(DEFAULT_DAEMON_NAME, DaemonScope::System)
    }

    pub fn user() -> Result<Self, DaemonError> {
        Self::try_acquire_lock(DEFAULT_DAEMON_NAME, DaemonScope::User)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::{assert_eq, assert_matches};
    use tokio::time::timeout;

    use crate::{
        DaemonId,
        msg::{Request, Response},
        test_utils::*,
    };

    #[tokio::test]
    async fn connect_multiple_clients() {
        const CLIENTS: usize = 10;

        let id = DaemonId::random();
        let listener = spawn_listener(&id, CLIENTS).await;
        let clients = spawn_clients(&id, CLIENTS, None).await;

        assert_no_err!(clients);
        assert_join_ok!(Some(listener));
    }

    #[tokio::test]
    async fn server_echoes_message() {
        let id = DaemonId::random();
        let listener = spawn_echo_listener(&id, 1).await;
        let mut client = spawn_clients(&id, 1, None).await.remove(0).unwrap();

        let request = Request {
            kind: 42,
            data: "ping".to_owned(),
        };
        let response = timeout(millis(1500), client.send(request.clone())).await;

        assert_matches!(response, Ok(Ok(Response::Echo(_))));
        let Ok(Ok(Response::Echo(response))) = response else {
            unreachable!();
        };
        assert_eq!(request, response);
        let listener = assert_join_ok!(Some(listener));
        let _ = assert_join_ok!(flatten!(listener));
    }
}
