// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{ffi::OsString, fmt, io};

use crate::{DaemonCx, DaemonId};
#[cfg(all(windows, feature = "service"))]
use crate::service::DaemonServiceError;

#[derive(Debug)]
pub enum DaemonError {
    InvalidName(OsString),
    AlreadyRunning(DaemonId),
    RundirUnavailable(DaemonId, io::Error),
    LockFailed(DaemonId, Box<dyn std::error::Error>),
    ConnectFailed(DaemonCx, io::Error),
    // SystemFailure(io::Error),
    #[cfg(all(windows, feature = "service"))]
    ServiceFailure(DaemonServiceError),
}

// impl From<io::Error> for DaemonError {
//     fn from(e: io::Error) -> Self {
//         Self::SystemFailure(e.into())
//     }
// }

#[cfg(all(windows, feature = "service"))]
impl From<DaemonServiceError> for DaemonError {
    fn from(e: DaemonServiceError) -> Self {
        Self::ServiceFailure(e.into())
    }
}

impl fmt::Display for DaemonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::InvalidName(ref name) => {
                write!(f, "invalid name: {}", name.display())
            }
            Self::AlreadyRunning(ref id) => {
                write!(f, "already running: {id}")
            }
            Self::RundirUnavailable(ref id, ref err) => {
                write!(f, "rundir unavailable: {id}, {err}")
            }
            Self::LockFailed(ref id, ref err) => {
                write!(f, "lock failed: {id}: {err}")
            }
            Self::ConnectFailed(ref cx, ref err) => {
                write!(f, "connect failed: {}: {err}", cx.id)
            }
            // Self::SystemFailure(ref err) => fmt::Display::fmt(err, f),
            #[cfg(all(windows, feature = "service"))]
            Self::ServiceFailure(ref err) => fmt::Display::fmt(err, f),
        }
    }
}

impl std::error::Error for DaemonError {}
