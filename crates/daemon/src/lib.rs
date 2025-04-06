// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

#![feature(once_cell_try_insert)]
#![feature(box_into_inner)]
#![feature(ptr_as_ref_unchecked)]
#![feature(iterator_try_collect)]
#![feature(new_zeroed_alloc)]

mod backend;
mod codec;
mod error;
mod id;
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
    backend::{DaemonClient, DaemonListener, DaemonLock, DaemonServer},
    error::DaemonError,
    id::{DaemonId, DaemonScope},
};

const DEFAULT_DAEMON_NAME: &str = "fractory-daemon";

#[cfg(any(test, feature = "rand"))]
pub fn random_name() -> std::ffi::OsString {
    use rand::{Rng, distr::Alphanumeric};

    let suffix = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect::<String>();

    os_str_concat!(DEFAULT_DAEMON_NAME, "-", suffix)
}
