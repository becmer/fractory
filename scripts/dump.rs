#!/usr/bin/env rust-script
//! ```cargo
//! [package]
//! edition = "2024"
//! [dependencies]
//! anyhow = "1"
//! globset = "0.4"
//! ignore = "0.4"
//! memchr = "2"
//! ```
#![feature(file_buffered)]

use std::{
    borrow::Cow,
    collections::BTreeMap,
    ffi::OsStr,
    fmt,
    fs::File,
    io::Write,
    iter::FromIterator,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use memchr::memchr;

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| Path::new("..").canonicalize().unwrap());
static IGNORE: LazyLock<GlobSet> = LazyLock::new(|| {
    const PATTERNS: &[&str] = &["assets/**", "LICENSE*", "Cargo.lock", ".editorconfig", ".gitattributes", ".gitignore", ".github/FUNDING.yml"];
    let mut builder = GlobSetBuilder::new();
    for pattern in PATTERNS {
        builder.add(Glob::new(pattern).unwrap());
    }
    builder.build().unwrap()
});

#[derive(Debug)]
struct Node {
    inner: BTreeMap<String, Self>,
}
impl Default for Node {
    fn default() -> Self {
        let inner = BTreeMap::<String, Self>::new();
        Self { inner }
    }
}
impl Node {
    fn push<'a, I>(&mut self, path: I)
    where
        I: IntoIterator<Item = &'a OsStr>,
    {
        let mut path = path.into_iter();
        let Some(node) = path.next().and_then(OsStr::to_str) else {
            return;
        };
        let node = self.inner.entry(node.to_string()).or_default();
        node.push(path);
    }
    fn iter(&self) -> impl Iterator<Item = (&String, &Self)> {
        self.inner
            .iter()
            .filter(|(_, v)| !v.inner.is_empty())
            .chain(self.inner.iter().filter(|(_, v)| v.inner.is_empty()))
    }
    fn fmt_with_prefix(&self, f: &mut fmt::Formatter<'_>, prefix: &str) -> fmt::Result {
        let mut entries = self.iter().peekable();
        while let Some((name, node)) = entries.next() {
            let is_last = entries.peek().is_none();
            let (branch, padding) = if is_last {
                ("└── ", "    ")
            } else {
                ("├── ", "│   ")
            };
            writeln!(f, "{prefix}{branch}{name}")?;
            if !node.inner.is_empty() {
                node.fmt_with_prefix(f, &format!("{prefix}{padding}"))?;
            }
        }
        Ok(())
    }
}
impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_with_prefix(f, "")
    }
}
impl<'a, P> FromIterator<&'a P> for Node
where
    P: AsRef<Path> + ?Sized,
{
    fn from_iter<I: IntoIterator<Item = &'a P>>(iter: I) -> Self {
        let mut node = Self::default();
        node.extend(iter);
        node
    }
}
impl<'a, P> Extend<&'a P> for Node
where
    P: AsRef<Path> + ?Sized,
{
    fn extend<T: IntoIterator<Item = &'a P>>(&mut self, iter: T) {
        for path in iter {
            self.push(path.as_ref());
        }
    }
}

fn filter_entry<P: AsRef<Path>>(path: P) -> bool {
    match path.as_ref().strip_prefix(&*ROOT) {
        Ok(path) => !IGNORE.is_match(path),
        Err(_) => false,
    }
}

fn determine_syntax<P: AsRef<Path> + ?Sized>(path: &P) -> Cow<'_, str> {
    let path = path.as_ref();
    let Some(ext) = path.extension().and_then(OsStr::to_str) else {
        return Cow::Borrowed("");
    };
    let ext = ext.to_ascii_lowercase();
    match ext.as_str() {
        "askama" => Cow::Borrowed("askama"),
        "md" => Cow::Borrowed("markdown"),
        "rs" => Cow::Borrowed("rust"),
        "toml" => Cow::Borrowed("toml"),
        _ => Cow::Owned(format!(".{ext}")),
    }
}

struct Fence {
    len: usize,
}
impl Fence {
    fn determine(mut content: &str) -> Self {
        let mut len = 0;
        while !content.is_empty() {
            let Some(p) = memchr(b'`', content.as_bytes()) else {
                break;
            };
            content = &content[p..];
            let local_len = content.bytes().take_while(|b| *b == b'`').count();
            content = &content[local_len..];
            len = local_len.max(len);
        }
        len = (len + 1).max(3);
        Self { len }
    }
}
impl fmt::Display for Fence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fence = std::iter::repeat_n('`', self.len).collect::<String>();
        f.write_str(&fence)
    }
}

fn main() -> Result<()> {
    let mut entries = Vec::<PathBuf>::new();
    for entry in WalkBuilder::new(&*ROOT)
        .hidden(false)
        .filter_entry(|e| !(e.path().is_dir() && e.path().ends_with(".git")))
        .sort_by_file_path(Path::cmp)
        .build()
    {
        let entry = entry?.into_path();
        entries.push(entry);
    }

    let tree = entries
        .iter()
        .filter(|e| e.is_file())
        .map(|p| p.strip_prefix(&*ROOT).unwrap())
        .collect::<Node>();

    let mut dump = File::create_buffered(ROOT.join("dump.md"))?;
    writeln!(dump, "## Files tree")?;
    writeln!(dump, "```")?;
    write!(dump, "{tree}")?;
    writeln!(dump, "```")?;
    writeln!(dump)?;

    for entry in entries {
        if entry.is_dir() || !filter_entry(&entry) {
            continue;
        }
        let name = entry.strip_prefix(&*ROOT)?.to_string_lossy();
        let syntax = determine_syntax(&entry);
        let content = std::fs::read_to_string(&entry)?;
        let fence = Fence::determine(&content);

        writeln!(dump, "### `{name}`")?;
        writeln!(dump, "{fence}{syntax}")?;
        write!(dump, "{content}")?;
        writeln!(dump, "{fence}")?;
        writeln!(dump)?;
    }

    Ok(())
}
