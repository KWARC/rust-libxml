//! Document canonicalization logic
//!
use std::ffi::{c_int, c_void, CString};
use std::os::raw;
use std::ptr::null_mut;

use crate::tree::c14n::*;

use super::{
  xmlAllocOutputBuffer, xmlC14NExecute, xmlC14NIsVisibleCallback, xmlChar, xmlNodePtr,
  xmlOutputBufferClose, xmlOutputBufferPtr, Document,
};

impl Document {
  /// Canonicalize a document and return the results.
  pub fn canonicalize(
    &self,
    options: CanonicalizationOptions,
    callback: Option<(xmlNodePtr, xmlC14NIsVisibleCallback)>,
  ) -> Result<String, ()> {
    let document = (*self.0).borrow().doc_ptr;

    let mut ns_list_c = to_xml_string_vec(options.inclusive_ns_prefixes);
    let inclusive_ns_prefixes = ns_list_c.as_mut_ptr();
    let with_comments = c_int::from(options.with_comments);

    let (is_visible_callback, user_data) = if let Some((node_ptr, visibility_callback)) = callback {
      (visibility_callback, node_ptr as *mut _)
    } else {
      (None, null_mut())
    };

    let mode = options.mode.into();
    unsafe {
      let c_obuf = create_output_buffer();

      let status = xmlC14NExecute(
        document,
        is_visible_callback,
        user_data,
        mode,
        inclusive_ns_prefixes,
        with_comments,
        c_obuf,
      );

      let res = c_obuf_into_output(c_obuf);

      if status < 0 {
        Err(())
      } else {
        Ok(res)
      }
    }
  }
}

unsafe fn c_obuf_into_output(c_obuf: xmlOutputBufferPtr) -> String { unsafe {
  let ctx_ptr = (*c_obuf).context;
  let output = Box::from_raw(ctx_ptr as *mut String);

  (*c_obuf).context = std::ptr::null_mut::<c_void>();

  xmlOutputBufferClose(c_obuf);

  *output
}}

unsafe fn create_output_buffer() -> xmlOutputBufferPtr { unsafe {
  let output = String::new();
  let ctx_ptr = Box::into_raw(Box::new(output));
  let encoder = std::ptr::null_mut();

  let buf = xmlAllocOutputBuffer(encoder);

  (*buf).writecallback = Some(xml_write_io);
  (*buf).closecallback = Some(xml_close_io);
  (*buf).context = ctx_ptr as _;

  buf
}}

unsafe extern "C" fn xml_close_io(_context: *mut raw::c_void) -> raw::c_int {
  0
}

unsafe extern "C" fn xml_write_io(
  io_ptr: *mut raw::c_void,
  buffer: *const raw::c_char,
  len: raw::c_int,
) -> raw::c_int { unsafe {
  if io_ptr.is_null() {
    0
  } else {
    let buf = std::slice::from_raw_parts_mut(buffer as *mut u8, len as usize);
    let buf = String::from_utf8_lossy(buf);
    let s2_ptr = io_ptr as *mut String;
    String::push_str(&mut *s2_ptr, &buf);

    len
  }
}}

/// Create a [Vec] of null-terminated [*mut xmlChar] strings
fn to_xml_string_vec(vec: Vec<String>) -> Vec<*mut xmlChar> {
  vec
    .into_iter()
    .map(|s| CString::new(s).unwrap().into_raw() as *mut xmlChar)
    .chain(std::iter::once(std::ptr::null_mut()))
    .collect()
}
