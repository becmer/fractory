// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

#![feature(array_try_map)]
#![feature(once_cell_try)]
#![feature(once_cell_try_insert)]
#![feature(string_from_utf8_lossy_owned)]

use std::{
    cell::{OnceCell, RefCell},
    collections::BTreeSet,
    ffi::OsStr,
    io,
    io::Read,
    os::windows::process::ExitStatusExt,
    process::{Child, ExitStatus, Stdio},
    thread,
    time::Duration,
};

use fractory_daemon::random_name;

const NUMBER_OF_DAEMONS: usize = 10;

#[test]
fn single_daemon_only() {
    let name = random_name();

    let mut running = Vec::from(
        thread::scope(|scope| {
            [(); NUMBER_OF_DAEMONS]
                .map(|_| scope.spawn_daemon(&name))
                .try_map(|handle| handle.join().expect("thread panicked"))
        })
        .expect("daemon process failed to start"),
    );

    let mut completed = Vec::<DaemonProcess>::new();

    while running.len() > 1 {
        let idx = running.iter().position(DaemonProcess::completed);
        match idx {
            Some(idx) => completed.push(running.swap_remove(idx)),
            None => thread::sleep(Duration::from_millis(100)),
        }
    }

    assert_eq!(
        (running.len(), completed.len()),
        (1, NUMBER_OF_DAEMONS - 1),
        "(running, completed)"
    );

    let running = running.into_iter().next().unwrap();
    running.kill();
    completed.push(running);

    let (stderr, failed) = completed
        .into_iter()
        .map(DaemonProcess::finish)
        .fold(
            (BTreeSet::<String>::new(), 0_usize),
            |(mut lines, failed), (status, stderr)| {
                for line in stderr.lines() {
                    lines.insert(line.to_owned());
                }
                (lines, failed + (!status.success()) as usize)
            },
        );

    for line in stderr {
        println!("{line}");
    }

    assert_eq!(failed, NUMBER_OF_DAEMONS - 1);
}

trait SpawnDaemon<'scope> {
    fn spawn_daemon<S: AsRef<OsStr>>(
        &'scope self,
        name: &'scope S,
    ) -> thread::ScopedJoinHandle<'scope, io::Result<DaemonProcess>>
    where
        S: Send + Sync + ?Sized + 'scope;
}
impl<'scope, 'env: 'scope> SpawnDaemon<'scope> for thread::Scope<'scope, 'env> {
    fn spawn_daemon<S: AsRef<OsStr>>(
        &'scope self,
        name: &'scope S,
    ) -> thread::ScopedJoinHandle<'scope, io::Result<DaemonProcess>>
    where
        S: Send + Sync + ?Sized + 'scope,
    {
        self.spawn(move || DaemonProcess::spawn(name))
    }
}

struct DaemonProcess {
    child: RefCell<Child>,
    status: OnceCell<ExitStatus>,
}

impl DaemonProcess {
    fn spawn<S: AsRef<OsStr>>(name: S) -> io::Result<Self> {
        let path = assert_cmd::cargo::cargo_bin("dummy-daemon");
        if !path.is_file() {
            panic!("`{}` not found", path.display());
        }
        let mut cmd = std::process::Command::new(path);
        cmd.arg(name);
        cmd.env("RUST_LOG", "debug");
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());
        Ok(Self {
            child: RefCell::new(cmd.spawn()?),
            status: OnceCell::new(),
        })
    }

    fn completed(&self) -> bool {
        self.status().is_some()
    }

    fn kill(&self) {
        if self.status().is_none() {
            let _ = self.status.try_insert(ExitStatus::default());
        }
        if self.child.borrow_mut().kill().is_err() {
            log::warn!("daemon failed to die");
        }
    }

    fn finish(self) -> (ExitStatus, String) {
        let status = self.status.into_inner().unwrap_or(ExitStatus::from_raw(1));
        let mut output = Vec::<u8>::new();
        if let Some(mut stderr) = self.child.into_inner().stderr {
            let _ = stderr.read_to_end(&mut output);
        }
        let output = String::from_utf8_lossy_owned(output);
        (status, output)
    }

    fn status(&self) -> Option<&ExitStatus> {
        let status = self
            .status
            .get_or_try_init(|| match self.child.borrow_mut().try_wait() {
                Ok(Some(status)) => Ok(status),
                Ok(None) => Err(None),
                Err(err) => Err(Some(err)),
            });
        match status {
            Ok(status) => Some(status),
            Err(None) => None,
            Err(Some(err)) => panic!("{err}"),
        }
    }
}
