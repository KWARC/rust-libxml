//! Namespace feature set
//!

use c_signatures::*;
use libc::c_void;
use std::ffi::{CStr, CString};
use std::ptr;
use std::str;

use tree::node::Node;

///An xml namespace
#[derive(Clone)]
pub struct Namespace {
  ///libxml's xmlNsPtr
  pub(crate) ns_ptr: *mut c_void,
}

impl Namespace {
  /// Creates a new namespace
  pub fn new(prefix: &str, href: &str, node: &mut Node) -> Result<Self, ()> {
    let c_href = CString::new(href).unwrap();
    let c_prefix = CString::new(prefix).unwrap();
    let c_prefix_ptr = if prefix.is_empty() {
      ptr::null()
    } else {
      c_prefix.as_ptr()
    };

    unsafe {
      let ns = xmlNewNs(node.node_ptr_mut(), c_href.as_ptr(), c_prefix_ptr);
      if ns.is_null() {
        Err(())
      } else {
        Ok(Namespace { ns_ptr: ns })
      }
    }
  }
  pub(crate) fn ns_ptr(&self) -> *mut c_void {
    self.ns_ptr
  }
  /// The namespace prefix
  pub fn get_prefix(&self) -> String {
    unsafe {
      let prefix_ptr = xmlNsPrefix(self.ns_ptr());
      if prefix_ptr.is_null() {
        String::new()
      } else {
        let c_prefix = CStr::from_ptr(prefix_ptr);
        c_prefix.to_string_lossy().into_owned()
      }
    }
  }

  /// The namespace href
  pub fn get_href(&self) -> String {
    unsafe {
      let href_ptr = xmlNsHref(self.ns_ptr());
      if href_ptr.is_null() {
        String::new()
      } else {
        let c_href = CStr::from_ptr(href_ptr);
        c_href.to_string_lossy().into_owned()
      }
    }
  }

  /// Explicit free method, until (if?) we implement automatic+safe free-on-drop
  pub fn free(&mut self) {
    unsafe { xmlFreeNs(self.ns_ptr()) }
  }
}
