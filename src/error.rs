//!
//! Wrapper for xmlError
//!
use super::bindings;

use std::ffi::CStr;


/// Wrapper around xmlErrorPtr
#[derive(Debug)]
pub struct StructuredError(*mut bindings::_xmlError);


impl StructuredError
{
    /// Wrap around and own a raw xmllib2 error structure
    pub fn from_raw(error: *mut bindings::_xmlError) -> Self
    {
        Self {0: error}
    }

    /// Human-readable informative error message
    pub fn message(&self) -> &str
    {
        let msg = unsafe{ CStr::from_ptr((*self.0).message) };

        msg.to_str().unwrap()
    }

    /// Return a raw pointer to the underlying xmlError structure
    pub fn as_ptr(&self) -> *const bindings::_xmlError
    {
        self.0  // we loose the *mut since we own it
    }
}


impl Drop for StructuredError
{
    fn drop(&mut self)
    {
        unsafe { bindings::xmlResetError(self.0) }
    }
}
