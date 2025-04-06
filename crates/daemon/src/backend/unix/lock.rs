// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{
    borrow::Cow,
    ffi::OsStr,
    fs::File,
    io::Write,
    mem::ManuallyDrop,
    path::{Path, PathBuf},
};

use nix::fcntl::{Flock, FlockArg};
use users::get_current_uid;

use crate::{DaemonError, DaemonId, DaemonScope};

pub struct DaemonLock {
    name: Cow<'static, OsStr>,
    flock: ManuallyDrop<Flock<File>>,
}

impl DaemonLock {
    pub fn try_acquire_lock<S: AsRef<OsStr>>(
        name: S,
        scope: DaemonScope,
    ) -> Result<Self, DaemonError> {
        let id = DaemonId::new(name, scope);
        log::debug!("going to acquire {id}");

        let path = mutex_path(&id);
        log::debug!("using flock path: {}", path.display());

        let mut flock = match File::options().write(true).create(true).open(path) {
            Ok(file) => match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
                Ok(flock) => flock,
                Err(_) => return Err(DaemonError::AlreadyRunning(id)),
            },
            Err(e) => return Err(DaemonError::from(e)),
        };

        match id.try_set() {
            Ok(id) => {
                log::debug!("successfully acquired {id}");
                let pid = std::process::id().to_string();
                flock.set_len(0)?;
                flock.write_all(pid.as_bytes())?;
                Ok(Self {
                    name: id.name,
                    flock: ManuallyDrop::new(flock),
                })
            }
            Err((locked, id)) => {
                log::debug!("already locked by {locked}");
                Err(id.into_already_running_error())
            }
        }
    }
}

impl Drop for DaemonLock {
    fn drop(&mut self) {
        log::debug!("going to release {}", self.name.to_string_lossy());
        unsafe { ManuallyDrop::drop(&mut self.flock) };
        log::debug!("successfully released {}", self.name.to_string_lossy());
    }
}

fn mutex_path(id: &DaemonId) -> PathBuf {
    match id.scope {
        DaemonScope::System => ["/run", "/var/run"]
            .into_iter()
            .map(Path::new)
            .find(Path::is_dir)
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(std::env::temp_dir()))
            .join(&id.name),
        DaemonScope::User => std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("{}-{}", &id.name, get_current_uid())),
    }
    .with_extension("pid")
}
