//! # A wrapper for libxml2
//! This library provides an interface to a subset of the libxml API.
//! The idea is to extend it whenever more functionality is needed.
//! Providing a more or less complete wrapper would be too much work.

extern crate libc;

mod c_signatures;
pub mod tree;
pub mod parser;
pub mod xpath;

