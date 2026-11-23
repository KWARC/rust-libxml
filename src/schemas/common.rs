//!
//! Common Utilities
//!
use crate::bindings;

use crate::error::StructuredError;

use std::ffi::c_void;

/// Provides a callback to the C side of things to accumulate xmlErrors to be
/// handled back on the Rust side.
#[cfg(libxml_older_than_2_12)]
pub unsafe extern "C" fn structured_error_handler(ctx: *mut c_void, error: bindings::xmlErrorPtr) {
  assert!(!ctx.is_null());
  let errlog = unsafe { &mut *{ ctx as *mut Vec<StructuredError> } };

  let error = unsafe { StructuredError::from_raw(error) };

  errlog.push(error);
}

#[cfg(not(libxml_older_than_2_12))]
pub unsafe extern "C" fn structured_error_handler(
  ctx: *mut c_void,
  error: *const bindings::xmlError,
) {
  assert!(!ctx.is_null());
  let errlog = unsafe { &mut *{ ctx as *mut Vec<StructuredError> } };

  let error = unsafe { StructuredError::from_raw(error) };

  errlog.push(error);
}

/// An error that libxml2 did not report, in the form of the errors it reports: fatal, in `domain`, with `code`.
pub(crate) fn unreported_error(
  domain: bindings::xmlErrorDomain,
  code: bindings::xmlParserErrors,
  message: String,
) -> Vec<StructuredError> {
  vec![StructuredError {
    message: Some(message),
    level: crate::error::XmlErrorLevel::Fatal,
    filename: None,
    line: None,
    col: None,
    domain: domain as i32,
    code: code as i32,
  }]
}

/// `path` as a C string, or the error for a NUL byte in it.
pub(crate) fn c_path(
  path: &str,
  domain: bindings::xmlErrorDomain,
) -> Result<std::ffi::CString, Vec<StructuredError>> {
  std::ffi::CString::new(path).map_err(|nul| {
    unreported_error(
      domain,
      bindings::xmlParserErrors_XML_ERR_INVALID_URI,
      format!("The path has a NUL byte at {}", nul.nul_position()),
    )
  })
}
