//!
//! Wrapping of the Parser Context (xmlSchemaParserCtxt)
//!
use super::common;

use crate::bindings;
use crate::error::StructuredError;
use crate::tree::document::Document;

use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::c_char;
use std::rc::Rc;

/// Wrapper on xmlSchemaParserCtxt
pub struct SchemaParserContext {
  inner: *mut bindings::_xmlSchemaParserCtxt,
  errlog: Rc<RefCell<Vec<StructuredError>>>,
}

impl SchemaParserContext {
  /// Create a schema parsing context from a Document object
  pub fn from_document(doc: &Document) -> Self {
    let parser = unsafe { bindings::xmlSchemaNewDocParserCtxt(doc.doc_ptr()) };

    if parser.is_null() {
      panic!("Failed to create schema parser context from XmlDocument"); // TODO error handling
    }

    Self::from_raw(parser)
  }

  /// Create a schema parsing context from a buffer in memory
  pub fn from_buffer<Bytes: AsRef<[u8]>>(buff: Bytes) -> Self {
    let buff_bytes = buff.as_ref();
    let buff_ptr = buff_bytes.as_ptr() as *const c_char;
    let buff_len = buff_bytes.len() as i32;

    let parser = unsafe { bindings::xmlSchemaNewMemParserCtxt(buff_ptr, buff_len) };

    if parser.is_null() {
      panic!("Failed to create schema parser context from buffer"); // TODO error handling
    }

    Self::from_raw(parser)
  }

  /// Create a schema parsing context from an URL
  pub fn from_file(path: &str) -> Self {
    let path = CString::new(path).unwrap(); // TODO error handling for \0 containing strings
    let path_ptr = path.as_bytes_with_nul().as_ptr() as *const i8;

    let parser = unsafe { bindings::xmlSchemaNewParserCtxt(path_ptr) };

    if parser.is_null() {
      panic!("Failed to create schema parser context from path"); // TODO error handling
    }

    Self::from_raw(parser)
  }

  /// Drains error log from errors that might have accumulated while parsing schema
  pub fn drain_errors(&mut self) -> Vec<StructuredError> {
    self.errlog.borrow_mut().drain(0..).collect()
  }

  /// Return a raw pointer to the underlying xmlSchemaParserCtxt structure
  pub fn as_ptr(&self) -> *mut bindings::_xmlSchemaParserCtxt {
    self.inner
  }
}

/// Private Interface
impl SchemaParserContext {
  fn from_raw(parser: *mut bindings::_xmlSchemaParserCtxt) -> Self {
    let errors = Rc::new(RefCell::new(Vec::new()));

    unsafe {
      bindings::xmlSchemaSetParserStructuredErrors(
        parser,
        Some(common::structured_error_handler),
        Box::into_raw(Box::new(Rc::downgrade(&errors))) as *mut _,
      );
    }

    Self {
      inner: parser,
      errlog: errors,
    }
  }
}

impl Drop for SchemaParserContext {
  fn drop(&mut self) {
    unsafe { bindings::xmlSchemaFreeParserCtxt(self.inner) }
  }
}
