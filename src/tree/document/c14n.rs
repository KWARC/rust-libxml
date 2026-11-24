//! Document canonicalization logic
//!
use std::ffi::{CString, NulError, c_int, c_void};
use std::os::raw;
use std::ptr::null_mut;

use crate::tree::c14n::*;

use super::{
  Document, xmlAllocOutputBuffer, xmlC14NExecute, xmlC14NIsVisibleCallback, xmlChar, xmlNodePtr,
  xmlOutputBufferClose, xmlOutputBufferPtr,
};

impl Document {
  /// Canonicalize a document and return the results.
  pub fn canonicalize(
    &self,
    options: CanonicalizationOptions,
    callback: Option<(xmlNodePtr, xmlC14NIsVisibleCallback)>,
  ) -> Result<String, ()> {
    let document = (*self.0).borrow().doc_ptr;

    // `ns_strings` owns the prefixes until the end of this call; libxml2 only reads them.
    let ns_strings = to_cstrings(options.inclusive_ns_prefixes).map_err(|_| ())?;
    let mut ns_list_c = to_xml_string_vec(&ns_strings);
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

      if status < 0 { Err(()) } else { Ok(res) }
    }
  }
}

unsafe fn c_obuf_into_output(c_obuf: xmlOutputBufferPtr) -> String {
  unsafe {
    // A NULL buffer (allocation failed in `create_output_buffer`) has no
    // captured context to recover — yield an empty canonicalization rather
    // than dereferencing address 0.
    if c_obuf.is_null() {
      return String::new();
    }
    let ctx_ptr = (*c_obuf).context;
    let output = Box::from_raw(ctx_ptr as *mut Vec<u8>);

    (*c_obuf).context = std::ptr::null_mut::<c_void>();

    xmlOutputBufferClose(c_obuf);

    // Decoded once, whole: libxml2 before 2.13 writes chunks that can end inside a UTF-8 sequence.
    String::from_utf8(*output)
      .unwrap_or_else(|invalid| String::from_utf8_lossy(invalid.as_bytes()).into_owned())
  }
}

unsafe fn create_output_buffer() -> xmlOutputBufferPtr {
  unsafe {
    let ctx_ptr = Box::into_raw(Box::new(Vec::<u8>::new()));
    let encoder = std::ptr::null_mut();

    let buf = xmlAllocOutputBuffer(encoder);
    // libxml2 returns NULL on allocation failure; reclaim the context box we
    // just leaked and propagate NULL rather than dereferencing address 0.
    // `xmlC14NExecute` treats a NULL buffer as an error (< 0), so the caller
    // surfaces `Err(())`.
    if buf.is_null() {
      drop(Box::from_raw(ctx_ptr));
      return buf;
    }

    (*buf).writecallback = Some(xml_write_io);
    (*buf).closecallback = Some(xml_close_io);
    (*buf).context = ctx_ptr as _;

    buf
  }
}

unsafe extern "C" fn xml_close_io(_context: *mut raw::c_void) -> raw::c_int {
  0
}

unsafe extern "C" fn xml_write_io(
  io_ptr: *mut raw::c_void,
  buffer: *const raw::c_char,
  len: raw::c_int,
) -> raw::c_int {
  if io_ptr.is_null() || buffer.is_null() || len <= 0 {
    return 0;
  }
  // SAFETY: libxml2 passes the `Vec` that `create_output_buffer` boxed, and `len` readable bytes at `buffer`.
  unsafe {
    let bytes = std::slice::from_raw_parts(buffer as *const u8, len as usize);
    (*(io_ptr as *mut Vec<u8>)).extend_from_slice(bytes);
  }
  len
}

/// `Err` if a string holds a NUL byte, which no C string does.
fn to_cstrings(vec: Vec<String>) -> Result<Vec<CString>, NulError> {
  vec.into_iter().map(CString::new).collect()
}

/// A NULL-terminated array of pointers into `strings`, which must outlive it.
/// (The strings used to be leaked through `CString::into_raw`.)
fn to_xml_string_vec(strings: &[CString]) -> Vec<*mut xmlChar> {
  strings
    .iter()
    .map(|s| s.as_ptr() as *mut xmlChar)
    .chain(std::iter::once(std::ptr::null_mut()))
    .collect()
}

#[cfg(test)]
mod tests {
  use super::xml_write_io;
  use std::os::raw;
  use std::ptr;

  #[test]
  fn write_callback_takes_nothing_from_a_null_or_empty_buffer() {
    let mut output = Vec::<u8>::new();
    let context = &mut output as *mut Vec<u8> as *mut raw::c_void;
    unsafe {
      assert_eq!(xml_write_io(context, ptr::null(), 0), 0);
      assert_eq!(xml_write_io(context, c"abc".as_ptr(), -1), 0);
    }
    assert!(output.is_empty());
  }
}
