//!
//! Common Utilities
//!
use crate::bindings;

use crate::error::StructuredError;

use std::rc::Weak;
use std::ffi::c_void;
use std::cell::RefCell;


/// Provides a callback to the C side of things to accumulate xmlErrors to be
/// handled back on the Rust side.
pub fn structured_error_handler(ctx: *mut c_void, error: bindings::xmlErrorPtr)
{
    let errlog = unsafe {
        Box::from_raw(ctx as *mut Weak<RefCell<Vec<StructuredError>>>)
    };

    let error = StructuredError::from_raw(error);

    if let Some(errors) = errlog.upgrade()
    {
        errors.borrow_mut()
            .push(error);
    } else {
        panic!("Underlying error log should not have outlived callback registration");
    }

    Box::leak(errlog);
}
