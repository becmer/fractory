// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{borrow::Cow, ffi::OsStr, fmt};

use crate::DaemonError;

static GLOBAL_DAEMON_ID: std::sync::OnceLock<DaemonId> = std::sync::OnceLock::new();

#[derive(Eq, PartialEq, Hash, Debug)]
pub struct DaemonId {
    pub name: Cow<'static, OsStr>,
    pub scope: DaemonScope,
}

impl From<&DaemonId> for DaemonId {
    fn from(id: &DaemonId) -> Self {
        id.clone()
    }
}

impl Clone for DaemonId {
    fn clone(&self) -> Self {
        Self {
            name: match self.name {
                Cow::Borrowed(name) => Cow::Borrowed(name),
                Cow::Owned(ref name) => Cow::Owned(name.clone()),
            },
            scope: self.scope,
        }
    }
}

impl DaemonId {
    fn to_borrowed(&'static self) -> Self {
        Self {
            name: Cow::Borrowed(&self.name),
            scope: self.scope,
        }
    }

    pub(crate) fn new<S: AsRef<OsStr>>(name: S, scope: DaemonScope) -> Self {
        Self {
            name: Cow::Owned(name.as_ref().to_os_string()),
            scope,
        }
    }

    pub(crate) fn try_set(self) -> Result<Self, (Self, Self)> {
        match GLOBAL_DAEMON_ID.try_insert(self) {
            Ok(global) => Ok(global.to_borrowed()),
            Err((global, this)) => Err((global.to_borrowed(), this)),
        }
    }

    #[must_use]
    pub(crate) fn try_get() -> Option<Self> {
        GLOBAL_DAEMON_ID.get().map(Self::to_borrowed)
    }

    #[must_use]
    pub(crate) fn get() -> Self {
        Self::try_get().expect("daemon id was not set")
    }

    #[must_use]
    pub(crate) fn into_invalid_name_error(self) -> DaemonError {
        DaemonError::InvalidName(self.name.into_owned())
    }

    #[must_use]
    pub(crate) fn into_already_running_error(self) -> DaemonError {
        DaemonError::AlreadyRunning(self)
    }
}

#[cfg(test)]
impl DaemonId {
    pub(crate) fn random() -> Self {
        Self::new(crate::random_name(), DaemonScope::User)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum DaemonScope {
    System,
    User,
}

impl fmt::Display for DaemonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.name.to_string_lossy(), self.scope)
    }
}

impl fmt::Display for DaemonScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::System => f.write_str("system"),
            Self::User => f.write_str("user"),
        }
    }
}
