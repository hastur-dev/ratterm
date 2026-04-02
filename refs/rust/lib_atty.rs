// atty Reference
// Cargo.toml: atty = "0.2"
// Usage: use atty::{Stream, is, isnt};

use atty::{Stream, is, isnt};

// ============================================================================
// ENUMS
// ============================================================================

pub enum Stream { }                      // possible stream sources
stream.clone()
stream.clone_from(source)
stream.fmt(f)
stream.type_id()
stream.borrow()
stream.borrow_mut()
stream.clone_to_uninit(dest)
stream.from(t)                           // Returns the argument unchanged.
stream.into()                            // Calls U::from(self).
stream.try_from(value)
stream.try_into()

pub fn is(stream: Stream) -> bool        // returns true if this is a tty
pub fn isnt(stream: Stream) -> bool      // returns true if this is not a tty

// ============================================================================
// FUNCTIONS
// ============================================================================

pub enum Stream { Stdout, Stderr, Stdin, } // possible stream sources
pub fn is(stream: Stream) -> bool        // returns true if this is a tty
pub fn isnt(stream: Stream) -> bool      // returns true if this is not a tty

