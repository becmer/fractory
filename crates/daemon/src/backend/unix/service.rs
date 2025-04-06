// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::sync::LazyLock;

use anyhow::{Error, Result};
use askama::Template;
use systemd::journal::JournalLog;
use tracing::error;

use crate::{DAEMON_NAME, DaemonCxPayload, DaemonScope};

/// Detects the absolute path to the currently running executable at runtime.
///
/// This value is used as the default `ExecStart=` in the generated systemd unit.
/// If resolution fails (e.g., due to container restrictions), it falls back to
/// `/usr/bin/fractory`.
///
/// The resolved path reflects the environment at the time of `fractory service generate`
/// or `fractory service install`. Users may edit the unit file afterward if necessary.
static SERVICE_PATH: LazyLock<String> = LazyLock::new(|| {
    std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| format!("/usr/bin/{DAEMON_NAME}"))
});

/// Renders a `fractory.service` systemd unit file using the Askama templating engine.
///
/// This unit file is suitable for both system-wide and per-user daemon installations.
/// The rendered output includes conditional `User=` directives, restart policies, and
/// proper `ExecStart=` configuration. The daemon explicitly manages socket handling.
#[derive(Debug, Template)]
#[template(path = "fractory.service.askama", escape = "none")]
pub struct ServiceUnit<'a> {
    /// Absolute path to the fractory binary (used as `ExecStart=`)
    pub exec_start: &'a str,
    /// Indicates whether this is a system-wide unit (`true`) or a per-user unit (`false`)
    pub is_system: bool,
    /// Optional user to run the daemon as (used as `User=`). Only set for system-wide units.
    pub run_as: Option<&'a str>,
}

impl ServiceUnit<'_> {
    /// Constructs a `ServiceUnit` from the provided daemon context.
    ///
    /// This determines whether the unit is system-wide or per-user by inspecting the context's scope.
    pub fn from_context<Cx: AsRef<DaemonCxPayload>>(cx: Cx) -> Self {
        let cx = cx.as_ref();
        Self::from_scope(cx.id.scope)
    }

    /// Constructs a `ServiceUnit` from a given `DaemonScope`.
    ///
    /// - For `DaemonScope::System`, includes `User=fractory`
    /// - For `DaemonScope::User`, omits `User=`
    pub fn from_scope(scope: DaemonScope) -> Self {
        match scope {
            DaemonScope::System => Self::system(),
            DaemonScope::User => Self::user(),
        }
    }
    /// Constructs a `ServiceUnit` for system-wide installation with `User=fractory`.
    ///
    /// Assumes the existence of a dedicated system user named `fractory`.
    pub fn system() -> Self {
        Self {
            is_system: true,
            run_as: Some(DAEMON_NAME),
            ..Self::user()
        }
    }
    /// Constructs a `ServiceUnit` for per-user installation, omitting `User=`.
    ///
    /// Intended for use with `systemd --user` mode under the current user's session.
    pub fn user() -> Self {
        Self {
            exec_start: &SERVICE_PATH,
            is_system: false,
            run_as: None,
        }
    }
}

pub struct DaemonService {
    _todo: (),
}

impl DaemonService {
    pub fn start() {
        if let Err(error) = JournalLog::init()
            .map_err(|e| Error::msg(e.to_string()))
            .and_then(|_| Self::try_start())
        {
            error!(%error, "{DAEMON_NAME} failed");
        }
    }

    pub fn try_start() -> Result<()> {
        let _rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use configparser::ini::Ini;
    use ntest::test_case;
    use pretty_assertions::assert_matches;

    use super::*;

    #[derive(Eq, PartialEq, Debug)]
    struct ParsedUnit {
        exec_start: Option<String>,
        user: Option<String>,
        wanted_by: Option<String>,
    }

    impl ParsedUnit {
        fn parse(unit: &ServiceUnit) -> Self {
            let mut parser = Ini::new_cs();
            parser.read(format!("{unit}")).expect("should parse");

            Self {
                exec_start: parser.get("Service", "ExecStart"),
                user: parser.get("Service", "User"),
                wanted_by: parser.get("Install", "WantedBy"),
            }
        }

        fn new(exec_start: &str, user: &str, wanted_by: &str) -> Self {
            Self {
                exec_start: Some([&SERVICE_PATH, exec_start].join(" ").trim_end().to_string()),
                user: (!user.is_empty()).then(|| user.to_owned()),
                wanted_by: (!wanted_by.is_empty()).then(|| wanted_by.to_owned()),
            }
        }
    }

    #[test_case(
        "/usr/bin/fractory",
        name = "generate_service_unit_with_custom_exec_start_usr"
    )]
    #[test_case(
        "/opt/fractory/bin/fractory",
        name = "generate_service_unit_with_custom_exec_start_opt"
    )]
    fn generate_service_unit_with_custom_exec_start(exec_start: &str) {
        let unit = ServiceUnit {
            exec_start,
            ..ServiceUnit::user()
        }
        .render();

        assert_matches!(unit, Ok(_));
        let Ok(unit) = unit else { unreachable!() };

        let expected = format!("ExecStart={}", exec_start);
        let actual = unit.lines().find(|l| l == &expected);
        assert_matches!(actual, Some(_));
    }

    #[test_case(
        "foo",
        "--system",
        "multi-user.target",
        name = "generate_service_unit_with_custom_user"
    )]
    #[test_case("", "", "default.target", name = "generate_service_unit_without_user")]
    fn generate_service_unit_with_custom_user(
        in_run_as: &str,
        out_exec_start: &str,
        out_wanted_by: &str,
    ) {
        let unit = ServiceUnit {
            is_system: !in_run_as.is_empty(),
            run_as: if !in_run_as.is_empty() {
                Some(in_run_as)
            } else {
                None
            },
            ..ServiceUnit::user()
        };

        let expected = ParsedUnit::new(out_exec_start, in_run_as, out_wanted_by);
        let actual = ParsedUnit::parse(&unit);

        assert_eq!(actual, expected);
    }
}
