// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{ffi::OsString, fmt, io};

use crate::DaemonId;
#[cfg(all(windows, feature = "service"))]
use crate::service::DaemonServiceError;

#[derive(Debug)]
pub enum DaemonError {
    InvalidName(OsString),
    AlreadyRunning(DaemonId),
    SystemFailure(io::Error),
    #[cfg(all(windows, feature = "service"))]
    ServiceFailure(DaemonServiceError),
}

impl From<io::Error> for DaemonError {
    fn from(e: io::Error) -> Self {
        Self::SystemFailure(e.into())
    }
}

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
                write!(f, "invalid daemon name: {}", name.to_string_lossy())
            }
            Self::AlreadyRunning(ref id) => {
                write!(f, "daemon already running: {id}")
            }
            Self::SystemFailure(ref err) => fmt::Display::fmt(err, f),
            #[cfg(all(windows, feature = "service"))]
            Self::ServiceFailure(ref err) => fmt::Display::fmt(err, f),
        }
    }
}

impl std::error::Error for DaemonError {}
