use std::{
    ffi::{OsStr, OsString},
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::OnceLock,
};

use tracing::{error, info, instrument, trace, warn};

use crate::{DAEMON_NAME, DaemonCxAttachment, DaemonError};

static GLOBAL_DAEMON_CX: OnceLock<DaemonCxPayload> = OnceLock::new();

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum DaemonScope {
    System,
    User,
}

impl fmt::Display for DaemonScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::System => f.write_str("system"),
            Self::User => f.write_str("user"),
        }
    }
}

pub struct DaemonId {
    pub scope: DaemonScope,
    pub name: OsString,
}

impl Clone for DaemonId {
    fn clone(&self) -> Self {
        Self {
            scope: self.scope,
            name: self.name.clone(),
        }
    }
}

impl PartialEq for DaemonId {
    fn eq(&self, other: &Self) -> bool {
        self.scope == other.scope && self.name == other.name
    }
}

impl Eq for DaemonId {}

impl Hash for DaemonId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.scope.hash(state);
        self.name.hash(state);
    }
}

impl fmt::Debug for DaemonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaemonId")
            .field("scope", &self.scope)
            .field("name", &self.name)
            .finish()
    }
}

impl fmt::Display for DaemonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.name.display(), self.scope)
    }
}

#[derive(Clone, Debug)]
pub struct DaemonCxPayload {
    pub id: DaemonId,
    attachment: DaemonCxAttachment,
}

impl DaemonCxPayload {
    pub fn system() -> Self {
        Self::from_scope(DaemonScope::System)
    }

    pub fn user() -> Self {
        Self::from_scope(DaemonScope::User)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn from_scope(scope: DaemonScope) -> Self {
        Self::new_(scope, DAEMON_NAME).expect("default name should be valid")
    }

    #[cfg(any(test, feature = "integration-tests"))]
    pub fn new<S: AsRef<OsStr>>(scope: DaemonScope, name: S) -> Result<Self, DaemonError> {
        Self::new_(scope, name)
    }

    #[instrument(level = "trace", skip(name))]
    fn new_<S: AsRef<OsStr>>(scope: DaemonScope, name: S) -> Result<Self, DaemonError> {
        let name = name.as_ref().to_os_string();
        let id = DaemonId { scope, name };
        trace!(%id, "creating new daemon context payload");
        let attachment = DaemonCxAttachment::new(&id)?;
        Ok(Self { id, attachment })
    }

    #[instrument(level = "trace", skip(self))]
    pub(crate) fn install(self) -> DaemonCx {
        match GLOBAL_DAEMON_CX.try_insert(self) {
            Ok(payload) => {
                info!(%payload.id, "daemon context installed");
                DaemonCx { payload }
            }
            Err((active, attempted)) => {
                error!(
                    attempted = %attempted.id,
                    active = %active.id,
                    "second daemon context installation attempted while one is already active",
                );
                panic!(
                    // It is a hard logic bug if this happened
                    "attempted to install a second daemon context (id: {}) while one is already active (id: {})",
                    attempted.id, active.id
                )
            }
        }
    }

    #[cfg(windows)]
    #[must_use]
    pub(crate) fn into_invalid_name_error(self) -> DaemonError {
        DaemonError::InvalidName(self.id.name.to_os_string())
    }

    #[cfg(windows)]
    #[must_use]
    pub(crate) fn into_already_running_error(self) -> DaemonError {
        DaemonError::AlreadyRunning(self.id.clone())
    }

    #[cfg(any(test, feature = "integration-tests"))]
    #[instrument(level = "trace", skip_all)]
    pub(crate) fn random_in(scope: DaemonScope) -> Self {
        use rand::{Rng, distr::Alphanumeric};

        let suffix = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect::<String>();

        let name = crate::os_str_concat!(DAEMON_NAME, "-", suffix);
        Self::new_(scope, name).expect("random name should be valid")
    }

    #[cfg(all(not(test), feature = "integration-tests"))]
    #[instrument(level = "trace")]
    pub fn random_in_user_scope() -> Self {
        Self::random_in(DaemonScope::User)
    }

    #[cfg(any(test, feature = "integration-tests"))]
    #[instrument(level = "trace", skip_all)]
    pub(crate) fn intern(self) -> DaemonCx {
        TEST_ARENA.intern(self)
    }
}

impl AsRef<DaemonCxPayload> for DaemonCxPayload {
    fn as_ref(&self) -> &DaemonCxPayload {
        self
    }
}

impl Deref for DaemonCxPayload {
    type Target = DaemonCxAttachment;
    fn deref(&self) -> &Self::Target {
        &self.attachment
    }
}

#[derive(Copy, Clone, Debug)]
pub struct DaemonCx {
    payload: &'static DaemonCxPayload,
}

impl DaemonCx {
    pub fn get() -> DaemonCx {
        match GLOBAL_DAEMON_CX.get() {
            Some(payload) => {
                trace!(%payload.id, "retrieved daemon context");
                DaemonCx { payload }
            }
            None => {
                error!("daemon context requested but not installed");
                panic!("daemon context not installed")
            }
        }
    }

    #[cfg(any(test, feature = "integration-tests"))]
    pub fn random_in_user_scope() -> Self {
        DaemonCxPayload::random_in(DaemonScope::User).intern()
    }
}

impl AsRef<DaemonCxPayload> for DaemonCx {
    fn as_ref(&self) -> &DaemonCxPayload {
        self.payload
    }
}

impl Deref for DaemonCx {
    type Target = DaemonCxPayload;
    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

#[cfg(any(test, feature = "integration-tests"))]
static TEST_ARENA: DaemonCxArena = DaemonCxArena::new();

#[cfg(any(test, feature = "integration-tests"))]
type InnerArena = std::sync::LazyLock<parking_lot::Mutex<Vec<Box<DaemonCxPayload>>>>;

#[cfg(any(test, feature = "integration-tests"))]
struct DaemonCxArena {
    inner: InnerArena,
}

#[cfg(any(test, feature = "integration-tests"))]
impl DaemonCxArena {
    const fn new() -> Self {
        use parking_lot::Mutex;
        let inner = InnerArena::new(|| Mutex::new(Vec::<Box<DaemonCxPayload>>::new()));
        Self { inner }
    }

    fn intern(&self, payload: DaemonCxPayload) -> DaemonCx {
        trace!(%payload.id, "interning daemon context");
        let mut guard = self.inner.lock();
        guard.push(Box::new(payload));
        let payload = &raw const **guard.last().unwrap();
        let payload = unsafe { payload.as_ref_unchecked() };
        let cx = DaemonCx { payload };
        trace!(?cx, "context interned");
        cx
    }
}
