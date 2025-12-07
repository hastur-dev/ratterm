//! File system entry types.
//!
//! Represents files and directories in the file browser.

use std::path::PathBuf;

/// Type of file system entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Regular file.
    File,
    /// Directory.
    Directory,
    /// Parent directory (../).
    ParentDir,
}

/// A file system entry (file or directory).
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Full path to the entry.
    path: PathBuf,
    /// Name of the entry (file/folder name).
    name: String,
    /// Type of entry.
    kind: EntryKind,
    /// File extension (for files only).
    extension: Option<String>,
    /// File size in bytes (for files only).
    size: Option<u64>,
}

impl FileEntry {
    /// Creates a new file entry.
    pub fn new(path: PathBuf, kind: EntryKind) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let extension = if kind == EntryKind::File {
            path.extension().map(|e| e.to_string_lossy().to_string())
        } else {
            None
        };

        Self {
            path,
            name,
            kind,
            extension,
            size: None,
        }
    }

    /// Creates a parent directory entry.
    pub fn parent_dir(path: PathBuf) -> Self {
        Self {
            path,
            name: "..".to_string(),
            kind: EntryKind::ParentDir,
            extension: None,
            size: None,
        }
    }

    /// Sets the file size.
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Returns the full path.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the entry name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the entry kind.
    #[must_use]
    pub fn kind(&self) -> EntryKind {
        self.kind
    }

    /// Returns the file extension.
    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        self.extension.as_deref()
    }

    /// Returns the file size.
    #[must_use]
    pub fn size(&self) -> Option<u64> {
        self.size
    }

    /// Returns true if this is a directory.
    #[must_use]
    pub fn is_directory(&self) -> bool {
        matches!(self.kind, EntryKind::Directory | EntryKind::ParentDir)
    }

    /// Returns true if this is a file.
    #[must_use]
    pub fn is_file(&self) -> bool {
        self.kind == EntryKind::File
    }

    /// Returns a display icon for the entry.
    #[must_use]
    pub fn icon(&self) -> &'static str {
        match self.kind {
            EntryKind::ParentDir => "..",
            EntryKind::Directory => "[D]",
            EntryKind::File => match self.extension.as_deref() {
                Some("rs") => "[R]",
                Some("py") => "[P]",
                Some("js" | "ts" | "jsx" | "tsx") => "[J]",
                Some("md") => "[M]",
                Some("toml" | "yaml" | "yml" | "json") => "[C]",
                Some("txt") => "[T]",
                _ => "[F]",
            },
        }
    }
}

/// Sorting options for file entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Name ascending (A-Z).
    #[default]
    NameAsc,
    /// Name descending (Z-A).
    NameDesc,
    /// Extension ascending.
    ExtensionAsc,
    /// Size ascending.
    SizeAsc,
    /// Size descending.
    SizeDesc,
}

impl SortOrder {
    /// Sorts a slice of file entries.
    pub fn sort(&self, entries: &mut [FileEntry]) {
        match self {
            Self::NameAsc => entries.sort_by(|a, b| {
                // Directories first, then files
                match (a.is_directory(), b.is_directory()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                }
            }),
            Self::NameDesc => entries.sort_by(|a, b| {
                match (a.is_directory(), b.is_directory()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => b.name.to_lowercase().cmp(&a.name.to_lowercase()),
                }
            }),
            Self::ExtensionAsc => entries.sort_by(|a, b| {
                match (a.is_directory(), b.is_directory()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.extension.cmp(&b.extension),
                }
            }),
            Self::SizeAsc => entries.sort_by(|a, b| {
                match (a.is_directory(), b.is_directory()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.size.cmp(&b.size),
                }
            }),
            Self::SizeDesc => entries.sort_by(|a, b| {
                match (a.is_directory(), b.is_directory()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => b.size.cmp(&a.size),
                }
            }),
        }
    }
}
