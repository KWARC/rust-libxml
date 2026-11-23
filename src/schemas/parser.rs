//!
//! Wrapping of the Parser Context (xmlSchemaParserCtxt)
//!
use super::common;

use crate::bindings;
use crate::error::StructuredError;
use crate::tree::document::Document;

use std::ffi::CString;
use std::os::raw::c_char;
use std::panic::AssertUnwindSafe;

/// Wrapper on xmlSchemaParserCtxt
pub struct SchemaParserContext {
  inner: *mut bindings::_xmlSchemaParserCtxt,
  errlog: *mut Vec<StructuredError>,
  /// The buffer or the document that `inner` reads the schema from when it is parsed, which libxml2 neither copies
  /// nor keeps alive: dropped after `inner` is freed. The document is only held, never read, so it leaves the
  /// context's auto traits as they were (`UnwindSafe`, `RefUnwindSafe`).
  _buffer: Option<Box<[u8]>>,
  _document: Option<AssertUnwindSafe<Document>>,
}

impl SchemaParserContext {
  /// Create a schema parsing context from a Document object, which it keeps alive until the context drops
  pub fn from_document(doc: &Document) -> Self {
    crate::init_parser();
    let parser = unsafe { bindings::xmlSchemaNewDocParserCtxt(doc.doc_ptr()) };

    if parser.is_null() {
      panic!("Failed to create schema parser context from XmlDocument"); // TODO error handling
    }

    Self::from_raw(parser, None, Some(AssertUnwindSafe(doc.clone())))
  }

  /// Create a schema parsing context from a buffer in memory, of which it keeps a copy until the context drops
  pub fn from_buffer<Bytes: AsRef<[u8]>>(buff: Bytes) -> Self {
    crate::init_parser();
    let buff_bytes: Box<[u8]> = buff.as_ref().into();
    let buff_ptr = buff_bytes.as_ptr() as *const c_char;
    let buff_len = buff_bytes.len() as i32;

    let parser = unsafe { bindings::xmlSchemaNewMemParserCtxt(buff_ptr, buff_len) };

    if parser.is_null() {
      panic!("Failed to create schema parser context from buffer"); // TODO error handling
    }

    Self::from_raw(parser, Some(buff_bytes), None)
  }

  /// Create a schema parsing context from an URL
  pub fn from_file(path: &str) -> Self {
    crate::init_parser();
    let path = CString::new(path).unwrap(); // TODO error handling for \0 containing strings
    let path_ptr = path.as_bytes_with_nul().as_ptr() as *const c_char;

    let parser = unsafe { bindings::xmlSchemaNewParserCtxt(path_ptr) };

    if parser.is_null() {
      panic!("Failed to create schema parser context from path"); // TODO error handling
    }

    Self::from_raw(parser, None, None)
  }

  /// Drains error log from errors that might have accumulated while parsing schema
  pub fn drain_errors(&mut self) -> Vec<StructuredError> {
    assert!(!self.errlog.is_null());
    let errors = unsafe { &mut *self.errlog };
    std::mem::take(errors)
  }

  /// Return a raw pointer to the underlying xmlSchemaParserCtxt structure
  pub fn as_ptr(&self) -> *mut bindings::_xmlSchemaParserCtxt {
    self.inner
  }
}

/// Private Interface
impl SchemaParserContext {
  fn from_raw(
    parser: *mut bindings::_xmlSchemaParserCtxt,
    buffer: Option<Box<[u8]>>,
    document: Option<AssertUnwindSafe<Document>>,
  ) -> Self {
    let errlog = Box::into_raw(Box::<Vec<StructuredError>>::default());

    unsafe {
      bindings::xmlSchemaSetParserStructuredErrors(
        parser,
        Some(common::structured_error_handler),
        errlog as *mut _,
      );
    }

    Self {
      inner: parser,
      errlog,
      _buffer: buffer,
      _document: document,
    }
  }
}

impl Drop for SchemaParserContext {
  fn drop(&mut self) {
    unsafe {
      bindings::xmlSchemaFreeParserCtxt(self.inner);
      if !self.errlog.is_null() {
        drop(Box::from_raw(self.errlog));
      }
    }
  }
}
