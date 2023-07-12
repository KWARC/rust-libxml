//!
//! Wrapper for xmlError
//!
use super::bindings;

use std::ffi::CStr;

/*
TODO:
The reason that the validation errors are duplicated is because libxml mutates
the same memory location for each error. This means that every StructuredError
object is actually wrapping the same pointer, and so every StructuredError ends
up with the contents of the last error that was generated.

My tentative plan is to rewrite StructuredError so that it contains all the same
fields as libxml's xmlError struct, and update the plumbing so that the errors
are copied into a new StructuredError object for each error that is generated.

After rummaging around in the libxml source code, I don't think we need to be calling
xmlResetError, at least in the new version where we'll be copying the data out into
a normal Rust struct.

In error.c, the __xmlRaiseError function appears to be what ultimately calls the
structured error handler (defined in schema/common, in this crate). __xmlRaiseError
does not allocate any new memory for the error it reports. Instead, it refers to a
a global: `xmlLastError`. In a threaded context, `xmlLastError` is buried inside
a "global state" struct that is unique per thread. Otherwise, xmlLastError is file
level variable that is globally shared through macro magic I don't quite grasp.

Point being, I don't think we need to call resetError as part of a Drop impl.
Even in a threaded context I don't think it'd make a difference, because internally
it calls memset instead of free, so it doesn't release any memory to the system.
*/

/// Wrapper around xmlErrorPtr
#[derive(Debug)]
pub struct StructuredError(*mut bindings::_xmlError);

impl StructuredError {
  /// Wrap around and own a raw xmllib2 error structure
  pub fn from_raw(error: *mut bindings::_xmlError) -> Self {
    Self(error)
  }

  /// Human-readable informative error message
  pub fn message(&self) -> &str {
    let msg = unsafe { CStr::from_ptr((*self.0).message) };

    msg.to_str().unwrap()
  }

  /// Return a raw pointer to the underlying xmlError structure
  pub fn as_ptr(&self) -> *const bindings::_xmlError {
    self.0 // we loose the *mut since we own it
  }
}

impl Drop for StructuredError {
  fn drop(&mut self) {
    unsafe { bindings::xmlResetError(self.0) }
  }
}
