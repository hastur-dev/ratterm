//! Error type for the durable store.
//!
//! Every fallible entry point in this module returns [`StoreError`]. Callers
//! upstream (the SSH collector, the push daemon, the dashboard) need to tell
//! "the disk is unusable" apart from "this database was written by a newer
//! build", because the first is worth retrying and the second is not.

use std::path::PathBuf;

use thiserror::Error;

/// Anything that can go wrong opening, writing to, or reading from the store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// SQLite refused the statement, the file, or the connection.
    ///
    /// This covers a corrupt or non-database file: SQLite only checks the
    /// header when the first page is read, so opening a garbage file succeeds
    /// and the first pragma or query fails with `NotADatabase`.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// The directory that should hold the database file could not be created.
    #[error("cannot create database directory {path}: {source}")]
    Directory {
        /// Directory that could not be created.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },

    /// The database on disk carries a schema version this build cannot use.
    ///
    /// Reported rather than silently migrated so an accidental downgrade does
    /// not rewrite a newer database in place.
    #[error("database schema version {found} is not supported by this build (expected {expected})")]
    SchemaVersion {
        /// Version read from `PRAGMA user_version`.
        found: i64,
        /// Version this build writes and reads.
        expected: i64,
    },

    /// No home directory is available, so the default database path is unknown.
    #[error("no home directory found for the default database location")]
    NoHomeDirectory,

    /// A caller passed an argument the store cannot act on.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// A value stored in the database did not round-trip back into its type.
    #[error("stored value for {field} could not be decoded: {detail}")]
    Decode {
        /// Column or field that failed to decode.
        field: &'static str,
        /// What went wrong.
        detail: String,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_message_names_both_versions() {
        let err = StoreError::SchemaVersion {
            found: 7,
            expected: 1,
        };
        let text = err.to_string();
        assert!(text.contains('7'), "message should name the found version");
        assert!(
            text.contains('1'),
            "message should name the expected version"
        );
    }

    #[test]
    fn sqlite_errors_convert_with_question_mark() {
        fn inner() -> Result<(), StoreError> {
            Err(rusqlite::Error::QueryReturnedNoRows)?;
            Ok(())
        }
        let err = inner().unwrap_err();
        assert!(matches!(err, StoreError::Sqlite(_)));
    }

    #[test]
    fn directory_error_keeps_the_path() {
        let err = StoreError::Directory {
            path: PathBuf::from("/nope/here"),
            source: std::io::Error::other("blocked"),
        };
        assert!(err.to_string().contains("nope"));
    }
}
