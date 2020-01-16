//!
//! Common Utilities
//!
use crate::bindings;

use crate::error::StructuredError;

use std::ffi::c_void;

/// Provides a callback to the C side of things to accumulate xmlErrors to be
/// handled back on the Rust side.
pub fn structured_error_handler(ctx: *mut c_void, error: bindings::xmlErrorPtr) {
  assert!(!ctx.is_null());
  let errlog = unsafe { &mut *{ ctx as *mut Vec<StructuredError> } };

  let error = StructuredError::from_raw(error);

  errlog.push(error);
}
