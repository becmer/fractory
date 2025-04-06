#![allow(dead_code)]

use std::{
    fmt, fs, io,
    io::{Read, Seek, Write},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use nix::{errno::Errno, fcntl::FlockArg};
use tracing::warn;

pub(super) type Pidfile = GenericPidfile<nix::fcntl::Flock<fs::File>>;

#[derive(Debug)]
pub(super) struct GenericPidfile<T> {
    pid: u32,
    path: PathBuf,
    flock: T,
}

#[allow(private_bounds)]
impl<T: Backend> GenericPidfile<T>
where
    T::Target: Input + Output + Sized,
{
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, PidfileError> {
        Self::open_with_pid(path, T::pid())
    }
    fn open_with_pid<P: AsRef<Path>>(path: P, pid: u32) -> Result<Self, PidfileError> {
        let path = path.as_ref().to_path_buf();

        let file = match T::open(Self::options(), &path) {
            Ok(file) => file,
            Err(source) => return Err(PidfileError::Inaccessible { path, source }),
        };

        match T::lock(file, FlockArg::LockExclusiveNonblock) {
            Ok(mut flock) => {
                let path = UnparsedPid::read(path, &mut *flock)?
                    .try_parse(pid)
                    .write(pid, &mut *flock)?;
                Ok(Self { pid, path, flock })
            }
            Err((mut file, Errno::EWOULDBLOCK)) => Err(UnparsedPid::read(path, &mut file)?
                .parse()?
                .into_already_locked_error(pid)),
            Err((_, errno)) => {
                let source = io::Error::from_raw_os_error(errno as io::RawOsError);
                Err(PidfileError::Inaccessible { path, source })
            }
        }
    }
    fn options() -> fs::OpenOptions {
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true);
        options
    }
}

#[derive(Debug)]
struct ParsedPid {
    path: PathBuf,
    pid: Option<u32>,
}
impl ParsedPid {
    fn parse(path: PathBuf, input: &[u8]) -> Result<Self, PidfileError> {
        if input.is_empty() {
            return Ok(Self { path, pid: None });
        }

        let result = match input.iter().all(u8::is_ascii_digit) {
            true => Ok(input),
            false => Err(io::Error::new(io::ErrorKind::InvalidData, "digit expected")),
        }
        .and_then(|input| std::str::from_utf8(input).or_invalid_data())
        .and_then(|s| s.parse::<u32>().or_invalid_data());

        match result {
            Ok(pid) => Ok(Self {
                path,
                pid: Some(pid),
            }),
            Err(source) => Err(PidfileError::Unreadable { path, source }),
        }
    }
    fn check_stale_pid(&self, current_pid: u32) {
        match self.pid {
            Some(written_pid) if written_pid != current_pid => {
                warn!(written_pid, current_pid, pidfile = %self.path.display(), "encountered stale pidfile");
            }
            _ => {}
        }
    }
    fn write<T: Output>(self, current_pid: u32, output: &mut T) -> Result<PathBuf, PidfileError> {
        match <T as Output>::write(output, current_pid) {
            Ok(_) => Ok(self.path),
            Err(source) => Err(PidfileError::OverwriteFailed {
                path: self.path,
                source,
            }),
        }
    }
    fn into_already_locked_error(self, current_pid: u32) -> PidfileError {
        PidfileError::AlreadyLocked {
            path: self.path,
            pid: match self.pid {
                Some(pid) if pid != current_pid => pid,
                _ => current_pid,
            },
        }
    }
}

struct UnparsedPid {
    path: PathBuf,
    data: Vec<u8>,
}
impl UnparsedPid {
    fn read<T: Input>(path: PathBuf, input: &mut T) -> Result<Self, PidfileError> {
        match <T as Input>::read(input) {
            Ok(data) => Ok(Self { path, data }),
            Err(source) => Err(PidfileError::Inaccessible { path, source }),
        }
    }
    fn parse(self) -> Result<ParsedPid, PidfileError> {
        ParsedPid::parse(self.path, self.data.trim_ascii())
    }
    fn try_parse(self, current_pid: u32) -> ParsedPid {
        match ParsedPid::parse(self.path, self.data.trim_ascii()) {
            Ok(parsed) => {
                parsed.check_stale_pid(current_pid);
                parsed
            }
            Err(error) => {
                warn!(current_pid, pidfile = %error.as_path().display(), %error, "encountered stale pidfile, unparsable");
                ParsedPid {
                    path: error.into_path(),
                    pid: None,
                }
            }
        }
    }
}

trait Backend: Deref + DerefMut + Sized
where
    Self::Target: Input + Output + Sized,
{
    fn pid() -> u32;
    fn open<P: AsRef<Path>>(opts: fs::OpenOptions, path: P) -> io::Result<Self::Target>;
    fn lock(t: Self::Target, args: FlockArg) -> Result<Self, (Self::Target, Errno)>;
}
impl Backend for nix::fcntl::Flock<fs::File> {
    fn pid() -> u32 {
        std::process::id()
    }
    fn open<P: AsRef<Path>>(opts: fs::OpenOptions, path: P) -> io::Result<fs::File> {
        opts.open(path)
    }
    fn lock(t: fs::File, args: FlockArg) -> Result<Self, (fs::File, Errno)> {
        nix::fcntl::Flock::lock(t, args)
    }
}

trait Input {
    fn read(&mut self) -> io::Result<Vec<u8>>;
}
impl<T: Read + Seek> Input for T {
    fn read(&mut self) -> io::Result<Vec<u8>> {
        self.rewind()?;
        let mut data = Vec::<u8>::new();
        self.read_to_end(&mut data)?;
        Ok(data)
    }
}

trait Output: Write + Seek {
    fn write(&mut self, pid: u32) -> io::Result<()> {
        let pid = pid.to_string();
        let pid = pid.as_bytes();
        self.rewind()?;
        self.clear()?;
        self.write_all(pid)?;
        self.sync()?;
        Ok(())
    }
    fn clear(&mut self) -> io::Result<()>;
    fn sync(&mut self) -> io::Result<()>;
}
impl Output for fs::File {
    fn clear(&mut self) -> io::Result<()> {
        self.set_len(0)
    }
    fn sync(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

trait OrInvalidData: Sized {
    type Output;
    fn or_invalid_data(self) -> io::Result<Self::Output>;
}
impl<T, E> OrInvalidData for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    type Output = T;
    fn or_invalid_data(self) -> io::Result<T> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        }
    }
}

#[derive(Debug)]
pub enum PidfileError {
    Inaccessible { path: PathBuf, source: io::Error },
    Unreadable { path: PathBuf, source: io::Error },
    AlreadyLocked { path: PathBuf, pid: u32 },
    OverwriteFailed { path: PathBuf, source: io::Error },
}
impl PidfileError {
    fn as_path(&self) -> &Path {
        match self {
            Self::Inaccessible { path, .. } => path,
            Self::Unreadable { path, .. } => path,
            Self::AlreadyLocked { path, .. } => path,
            Self::OverwriteFailed { path, .. } => path,
        }
    }
    fn into_path(self) -> PathBuf {
        match self {
            Self::Inaccessible { path, .. } => path,
            Self::Unreadable { path, .. } => path,
            Self::AlreadyLocked { path, .. } => path,
            Self::OverwriteFailed { path, .. } => path,
        }
    }
}
impl fmt::Display for PidfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Inaccessible {
                ref path,
                ref source,
            } => write!(f, "inaccessible pidfile at {}: {}", path.display(), source),
            Self::Unreadable {
                ref path,
                ref source,
            } => write!(f, "unreadable pidfile at {}: {}", path.display(), source),
            Self::AlreadyLocked { ref path, pid } => {
                write!(f, "pidfile already locked at {}: {pid}", path.display())
            }
            Self::OverwriteFailed {
                ref path,
                ref source,
            } => {
                write!(
                    f,
                    "pidfile overwrite failed at {}: {}",
                    path.display(),
                    source
                )
            }
        }
    }
}
impl std::error::Error for PidfileError {}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, fs::OpenOptions, io::Cursor};

    use pretty_assertions::assert_matches;

    use super::*;
    use crate::test_utils::record;

    thread_local! {
        static PID: RefCell<u32> = RefCell::new(0);
        static DATA: RefCell<io::Result<Vec<u8>>> = RefCell::new(Ok(Vec::new()));
        static LOCK: RefCell<Option<Errno>> = RefCell::new(None);
    }

    macro_rules! pid {
        ($pid:expr) => {{
            let pid = ($pid).to_string();
            pid.as_bytes().to_vec()
        }};
    }

    macro_rules! unparsed {
        ($pid:expr) => {
            UnparsedPid {
                path: PathBuf::new(),
                data: $pid,
            }
        };
    }

    macro_rules! parsed {
        ($pid:expr) => {
            ParsedPid {
                path: PathBuf::new(),
                pid: $pid,
            }
        };
    }

    fn setup(current_pid: u32, pidfile_data: io::Result<Vec<u8>>, lock_errno: Option<Errno>) {
        PID.set(current_pid);
        DATA.set(pidfile_data);
        LOCK.set(lock_errno);
    }

    type DummyFile = Cursor<Vec<u8>>;

    #[derive(Debug, Default)]
    struct DummyFlock {
        file: DummyFile,
    }
    impl Deref for DummyFlock {
        type Target = DummyFile;
        fn deref(&self) -> &Self::Target {
            &self.file
        }
    }
    impl DerefMut for DummyFlock {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.file
        }
    }
    impl Backend for DummyFlock {
        fn pid() -> u32 {
            PID.take()
        }
        fn open<P: AsRef<Path>>(_: OpenOptions, _: P) -> io::Result<Self::Target> {
            Ok(Cursor::new(DATA.replace(Ok(Vec::new()))?))
        }
        fn lock(file: DummyFile, _arg: FlockArg) -> Result<Self, (DummyFile, Errno)> {
            match LOCK.take() {
                None => Ok(DummyFlock { file }),
                Some(errno) => Err((file, errno)),
            }
        }
    }

    impl Output for DummyFile {
        fn clear(&mut self) -> io::Result<()> {
            self.get_mut().clear();
            Ok(())
        }
        fn sync(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    const CURRENT_PID: u32 = 123;
    const FOREIGN_PID: u32 = 456;
    const INVALID_DATA: &[u8] = b"invalid";

    #[test]
    fn parse_empty() {
        let result = ParsedPid::parse(PathBuf::new(), b"");
        assert_matches!(result, Ok(ParsedPid { pid: None, .. }));
    }

    #[test]
    fn parse_invalid_utf8() {
        let result = ParsedPid::parse(PathBuf::new(), b"\xff");
        assert_matches!(result, Err(PidfileError::Unreadable { .. }));
    }

    #[test]
    fn parse_garbage() {
        let result = ParsedPid::parse(PathBuf::new(), b"123 pid");
        assert_matches!(result, Err(PidfileError::Unreadable { .. }));
    }

    #[test]
    fn parse_negative_number() {
        let result = ParsedPid::parse(PathBuf::new(), b"-123");
        assert_matches!(result, Err(PidfileError::Unreadable { .. }));
    }

    #[test]
    fn parse_zero() {
        let result = ParsedPid::parse(PathBuf::new(), b"0");
        assert_matches!(result, Ok(ParsedPid { pid: Some(0), .. }));
    }

    #[test]
    fn parse_positive_number_signed() {
        let result = ParsedPid::parse(PathBuf::new(), b"+123");
        assert_matches!(result, Err(PidfileError::Unreadable { .. }));
    }

    #[test]
    fn parse_positive_number_unsigned() {
        let result = ParsedPid::parse(PathBuf::new(), b"123");
        assert_matches!(result, Ok(ParsedPid { pid: Some(123), .. }));
    }

    #[test]
    fn try_parse_different_pid_warns_stale() {
        let parsed = record(|| unparsed!(pid!(FOREIGN_PID)).try_parse(CURRENT_PID));
        assert!(parsed.logged("encountered stale pidfile"));
        assert_matches!(parsed.pid, Some(FOREIGN_PID));
    }

    #[test]
    fn try_parse_same_pid_do_not_warn() {
        let parsed = record(|| unparsed!(pid!(CURRENT_PID)).try_parse(CURRENT_PID));
        assert!(!parsed.logged("encountered stale pidfile"));
        assert_matches!(parsed.pid, Some(CURRENT_PID));
    }

    #[test]
    fn try_parse_empty_do_not_warn() {
        let parsed = record(|| unparsed!(Vec::new()).try_parse(CURRENT_PID));
        assert!(!parsed.logged("encountered stale pidfile"));
        assert_matches!(parsed.pid, None);
    }

    #[test]
    fn try_parse_unparseable_pid_warns_stale() {
        let parsed = record(|| unparsed!(INVALID_DATA.to_vec()).try_parse(CURRENT_PID));
        assert!(parsed.logged("encountered stale pidfile"));
        assert_matches!(parsed.pid, None);
    }

    #[test]
    fn into_already_locked_error_with_different_pid() {
        let error = parsed!(Some(FOREIGN_PID)).into_already_locked_error(CURRENT_PID);
        assert_matches!(
            error,
            PidfileError::AlreadyLocked {
                pid: FOREIGN_PID,
                ..
            }
        );
    }

    #[test]
    fn into_already_locked_error_with_none() {
        let error = parsed!(None).into_already_locked_error(CURRENT_PID);
        assert_matches!(
            error,
            PidfileError::AlreadyLocked {
                pid: CURRENT_PID,
                ..
            }
        );
    }

    #[test]
    fn pidfile_open_writes_pid() {
        setup(CURRENT_PID, Ok(pid!(FOREIGN_PID)), None);
        let pidfile = GenericPidfile::<DummyFlock>::open("").unwrap();
        assert_eq!(pidfile.pid, CURRENT_PID);
        assert_eq!(pidfile.flock.get_ref(), &pid!(CURRENT_PID));
    }

    #[test]
    fn pidfile_open_not_found() {
        setup(
            CURRENT_PID,
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no such file or directory",
            )),
            None,
        );
        let result = GenericPidfile::<DummyFlock>::open("");
        assert_matches!(result, Err(PidfileError::Inaccessible { .. }));
    }

    #[test]
    fn pidfile_open_already_locked() {
        setup(CURRENT_PID, Ok(pid!(FOREIGN_PID)), Some(Errno::EWOULDBLOCK));
        let result = GenericPidfile::<DummyFlock>::open("");
        assert_matches!(
            result,
            Err(PidfileError::AlreadyLocked {
                pid: FOREIGN_PID,
                ..
            })
        );
    }

    #[test]
    fn pidfile_open_other_lock_error() {
        setup(CURRENT_PID, Ok(pid!(FOREIGN_PID)), Some(Errno::EACCES));
        let result = GenericPidfile::<DummyFlock>::open("");
        assert_matches!(result, Err(PidfileError::Inaccessible { .. }));
    }
}
