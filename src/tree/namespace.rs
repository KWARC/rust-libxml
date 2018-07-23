//! Namespace feature set
//!

use bindings::*;
use c_helpers::*;

use std::error::Error;
use std::ffi::{CStr, CString};
use std::ptr;
use std::str;

use tree::node::Node;

///An xml namespace
#[derive(Clone)]
pub struct Namespace {
  ///libxml's xmlNsPtr
  pub(crate) ns_ptr: xmlNsPtr,
}

impl Namespace {
  /// Creates a new namespace
  pub fn new(prefix: &str, href: &str, node: &mut Node) -> Result<Self, Box<Error>> {
    let c_href = CString::new(href).unwrap();
    let c_prefix = CString::new(prefix).unwrap();
    let c_prefix_ptr = if prefix.is_empty() {
      ptr::null()
    } else {
      c_prefix.as_ptr()
    };

    unsafe {
      let ns = xmlNewNs(
        node.node_ptr_mut()?,
        c_href.as_bytes().as_ptr(),
        c_prefix_ptr as *const u8,
      );
      if ns.is_null() {
        Err(From::from("xmlNewNs returned NULL"))
      } else {
        Ok(Namespace { ns_ptr: ns })
      }
    }
  }

  /// Immutably borrows the underlying libxml2 `xmlNsPtr` pointer
  pub fn ns_ptr(&self) -> xmlNsPtr {
    self.ns_ptr
  }

  /// Mutably borrows the underlying libxml2 `xmlNsPtr` pointer
  pub fn ns_ptr_mut(&mut self) -> xmlNsPtr {
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
