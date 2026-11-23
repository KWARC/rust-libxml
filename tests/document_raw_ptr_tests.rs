//! `Document::from_raw_ptr` takes ownership of a libxml2 document made outside the crate, and frees it once, with its
//! last handle, as for a parsed document; the deprecated `new_ptr` does the same.
mod common;

use common::quarantine::{double_freed, is_freed, setup};
use libxml::bindings::{xmlDocPtr, xmlReadMemory};
use libxml::tree::Document;

fn read(xml: &str) -> xmlDocPtr {
  setup();
  let doc_ptr = unsafe {
    xmlReadMemory(
      xml.as_ptr() as *const _,
      xml.len() as _,
      std::ptr::null(),
      std::ptr::null(),
      0,
    )
  };
  assert!(!doc_ptr.is_null());
  doc_ptr
}

#[test]
fn from_raw_ptr_owns_the_document() {
  let doc_ptr = read("<r><c/></r>");
  // SAFETY: a new document, which only this `Document` holds.
  let doc = unsafe { Document::from_raw_ptr(doc_ptr) };
  assert_eq!(doc.doc_ptr(), doc_ptr);
  let child = doc.get_root_element().unwrap().get_first_child().unwrap();
  assert_eq!(child.get_name(), "c");
  drop(doc);
  // As for a parsed document: a node holds its document weakly, and reads as null once it is freed.
  assert!(is_freed(doc_ptr));
  assert!(!double_freed(doc_ptr));
  assert!(child.is_null());
  drop(child);
  assert!(!double_freed(doc_ptr));
}

#[test]
#[allow(deprecated)]
fn new_ptr_still_owns_the_document() {
  let doc_ptr = read("<r/>");
  let doc = Document::new_ptr(doc_ptr);
  assert_eq!(doc.get_root_element().unwrap().get_name(), "r");
  drop(doc);
  assert!(is_freed(doc_ptr));
  assert!(!double_freed(doc_ptr));
}

#[test]
fn from_raw_ptr_of_null_is_a_document_without_one() {
  // SAFETY: NULL is allowed.
  let doc = unsafe { Document::from_raw_ptr(std::ptr::null_mut()) };
  assert!(doc.doc_ptr().is_null());
  assert!(doc.get_root_element().is_none());
}
