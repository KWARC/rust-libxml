//! # A wrapper for libxml2
//! This library provides an interface to a subset of the libxml API.
//! The idea is to extend it whenever more functionality is needed.
//! Providing a more or less complete wrapper would be too much work.
#![deny(missing_docs)]
// Our new methods return Result<Self, _> types
#![allow(clippy::new_ret_no_self)]

/// Bindings to the C interface
pub mod bindings;
mod c_helpers;

/// XML and HTML parsing
pub mod parser;
/// Manipulations on the DOM representation
pub mod tree;
/// `XPath` module for global lookup in the DOM
pub mod xpath;
