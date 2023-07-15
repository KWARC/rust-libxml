//!
//! Wrapper for xmlError
//!
use super::bindings;

use std::ffi::{c_char, c_int, CStr};

/// Rust enum variant of libxml2's xmlErrorLevel
#[derive(Debug)]
pub enum XmlErrorLevel {
  /// No error
  None,
  /// A simple warning
  Warning,
  /// A recoverable error
  Error,
  /// A fatal error
  Fatal,
}

impl XmlErrorLevel {
  /// Convert an xmlErrorLevel provided by libxml2 (as an integer) into a Rust enum
  pub fn from_raw(error_level: bindings::xmlErrorLevel) -> XmlErrorLevel {
    match error_level {
      bindings::xmlErrorLevel_XML_ERR_NONE => XmlErrorLevel::None,
      bindings::xmlErrorLevel_XML_ERR_WARNING => XmlErrorLevel::Warning,
      bindings::xmlErrorLevel_XML_ERR_ERROR => XmlErrorLevel::Error,
      bindings::xmlErrorLevel_XML_ERR_FATAL => XmlErrorLevel::Fatal,
      _ => XmlErrorLevel::None, // TODO: What is the right fallback here?
    }
  }
}

/// Wrapper around xmlErrorPtr.
/// Some fields have been omitted for simplicity/safety
#[derive(Debug)]
pub struct StructuredError {
  /// Human-friendly error message, lossily converted into UTF-8 from the underlying
  /// C string. May be `None` if an error message is not provided by libxml2.
  pub message: Option<String>,
  /// The error's level
  pub level: XmlErrorLevel,
  /// The filename, lossily converted into UTF-8 from the underlying C string.
  /// May be `None` if a filename is not provided by libxml2, such as when validating
  /// an XML document stored entirely in memory.
  pub filename: Option<String>,
  /// The linenumber, or None if not applicable.
  pub line: Option<c_int>,
  /// The column where the error is present, or None if not applicable.
  pub col: Option<c_int>,

  /// The module that the error came from. See libxml's xmlErrorDomain enum.
  pub domain: c_int,
  /// The variety of error. See libxml's xmlParserErrors enum.
  pub code: c_int,
}

impl StructuredError {
  /// Copies the error information stored at `error_ptr` into a new `StructuredError`
  pub fn from_raw(error_ptr: *mut bindings::_xmlError) -> Self {
    unsafe {
      let error = *error_ptr;
      let message = StructuredError::convert_to_owned(error.message);
      let level = XmlErrorLevel::from_raw(error.level);
      let filename = StructuredError::convert_to_owned(error.file);

      let line = if error.line == 0 {
        None
      } else {
        Some(error.line)
      };
      let col = if error.int2 == 0 {
        None
      } else {
        Some(error.int2)
      };

      StructuredError {
        message,
        level,
        filename,
        line,
        col,
        domain: error.domain,
        code: error.code,
      }
    }
  }

  /// Returns the provided c_str as Some(String), or None if the provided pointer is null.
  unsafe fn convert_to_owned(c_str: *mut c_char) -> Option<String> {
    if c_str.is_null() {
      return None;
    }

    let raw_str = CStr::from_ptr(c_str);
    Some(String::from_utf8_lossy(raw_str.to_bytes()).to_string())
  }
}
