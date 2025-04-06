// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

#![feature(once_cell_try_insert)]
#![feature(box_into_inner)]
#![feature(ptr_as_ref_unchecked)]
#![feature(iterator_try_collect)]
#![feature(new_zeroed_alloc)]
#![feature(os_str_display)]
#![feature(impl_trait_in_assoc_type)]
#![feature(unboxed_closures)]
#![feature(raw_os_error_ty)]
#![cfg_attr(test, feature(once_cell_try))]
#![cfg_attr(test, feature(fn_traits))]

mod backend;
mod codec;
mod cx;
mod error;
pub mod msg;
#[cfg(test)]
mod test_utils;

macro_rules! os_str_concat {
    ($base:expr, $($other:expr),+ $(,)?) => {{
        let mut s = ::std::ffi::OsString::from($base);
        $(s.push($other);)+
        s
    }};
}

pub(crate) use os_str_concat;

#[cfg(feature = "service")]
pub use self::backend::service;
pub use self::{
    backend::{
        DaemonClient, DaemonConnector, DaemonCxAttachment, DaemonListener, DaemonLock,
        DaemonServer, NativeDaemonConnector, NativeDaemonListener, NativeDaemonServer,
    },
    cx::{DaemonCx, DaemonCxPayload, DaemonId, DaemonScope},
    error::DaemonError,
};

pub(crate) const DAEMON_NAME: &str = "fractory";
