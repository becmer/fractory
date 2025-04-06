// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    io,
};

use widestring::U16CString;
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
        System::Threading::CreateMutexW,
    },
    core::PCWSTR,
};

use crate::{DaemonError, DaemonId, DaemonScope, os_str_concat};

#[derive(Debug)]
pub struct DaemonLock {
    name: Cow<'static, OsStr>,
    handle: HANDLE,
}

impl DaemonLock {
    pub fn try_acquire_lock<S: AsRef<OsStr>>(
        name: S,
        scope: DaemonScope,
    ) -> Result<Self, DaemonError> {
        let id = DaemonId::new(name, scope);
        log::debug!("going to acquire {id}");

        let Ok(ucs_name) = U16CString::from_os_str(mutex_path(&id)) else {
            return Err(id.into_invalid_name_error());
        };
        let pcw_name = PCWSTR(ucs_name.as_ptr());

        let handle = unsafe { CreateMutexW(None, true, pcw_name).map_err(io::Error::from)? };
        if ERROR_ALREADY_EXISTS == unsafe { GetLastError() } {
            return Err(id.into_already_running_error());
        }

        match id.try_set() {
            Ok(id) => {
                log::debug!("successfully acquired {id}");
                Ok(DaemonLock {
                    name: id.name,
                    handle,
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
        let _ = unsafe { CloseHandle(self.handle) };
        log::debug!("successfully released {}", self.name.to_string_lossy());
    }
}

fn mutex_path(id: &DaemonId) -> OsString {
    let scope = match id.scope {
        DaemonScope::System => OsStr::new(r"Global\"),
        DaemonScope::User => OsStr::new(r"Local\"),
    };
    os_str_concat!(scope, &id.name)
}
