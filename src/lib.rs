//! # A wrapper for libxml2
//! This library provides an interface to a subset of the libxml API.
//! The idea is to extend it whenever more functionality is needed.
//! Providing a more or less complete wrapper would be too much work.
#![deny(missing_docs)]
// Our new methods return Result<Self, _> types
#![allow(clippy::new_ret_no_self, clippy::result_unit_err)]
/// Bindings to the C interface
pub mod bindings;
mod c_helpers;

/// XML and HTML parsing
pub mod parser;

/// Manipulations on the DOM representation
pub mod tree;

/// XML Global Error Structures and Handling
pub mod error;

/// `XPath` module for global lookup in the DOM
pub mod xpath;

/// Schema Validation
pub mod schemas;

/// Read-only parallel primitives
pub mod readonly;

/// Custom input callbacks for `xmlRegisterInputCallbacks` — bundle
/// XSLT stylesheets / RNG schemas inside the binary and serve them
/// through a user-defined URL scheme (e.g. `embed:///foo.xsl`).
pub mod io;

/// Ensure libxml2's global parser state is initialised. Safe to call from
/// any number of threads — internally guarded by `std::sync::Once` so the
/// underlying `xmlInitParser()` runs exactly once. Call this before
/// performing any libxml2 operations from application code that does
/// *not* go through the `parser::Parser` API (which initialises lazily).
///
/// See libxml2's own thread-safety guidance:
/// <https://dev.w3.org/XInclude-Test-Suite/libxml2-2.4.24/doc/threads.html>
pub fn init_parser() {
  use std::sync::Once;
  static INIT: Once = Once::new();
  INIT.call_once(|| unsafe {
    bindings::xmlInitParser();
  });
}
