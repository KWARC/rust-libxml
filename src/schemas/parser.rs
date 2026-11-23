//!
//! Wrapping of the Parser Context (xmlSchemaParserCtxt)
//!
use super::common;

use crate::bindings;
use crate::error::StructuredError;
use crate::tree::document::Document;

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
  ///
  /// # Panics
  /// Where [`try_from_document`](Self::try_from_document) returns an error.
  pub fn from_document(doc: &Document) -> Self {
    Self::try_from_document(doc).unwrap_or_else(|errors| {
      failed(
        "Failed to create schema parser context from XmlDocument",
        errors,
      )
    })
  }

  /// Create a schema parsing context from a Document object, which it keeps alive until the context drops
  ///
  /// Returns an error, where [`from_document`](Self::from_document) panics, for a `Document` without a libxml2
  /// document, and if libxml2 makes no context.
  pub fn try_from_document(doc: &Document) -> Result<Self, Vec<StructuredError>> {
    crate::init_parser();
    let doc_ptr = doc.doc_ptr();
    if doc_ptr.is_null() {
      return Err(parser_error(
        bindings::xmlParserErrors_XML_ERR_DOCUMENT_EMPTY,
        "The Document has no libxml2 document".to_string(),
      ));
    }
    let parser = unsafe { bindings::xmlSchemaNewDocParserCtxt(doc_ptr) };
    Self::checked(parser, None, Some(AssertUnwindSafe(doc.clone())))
  }

  /// Create a schema parsing context from a buffer in memory, of which it keeps a copy until the context drops
  ///
  /// # Panics
  /// Where [`try_from_buffer`](Self::try_from_buffer) returns an error.
  pub fn from_buffer<Bytes: AsRef<[u8]>>(buff: Bytes) -> Self {
    Self::try_from_buffer(buff)
      .unwrap_or_else(|errors| failed("Failed to create schema parser context from buffer", errors))
  }

  /// Create a schema parsing context from a buffer in memory, of which it keeps a copy until the context drops
  ///
  /// Returns an error, where [`from_buffer`](Self::from_buffer) panics, for an empty buffer, one of more than
  /// `i32::MAX` bytes, which libxml2 does not read from memory, and if libxml2 makes no context.
  pub fn try_from_buffer<Bytes: AsRef<[u8]>>(buff: Bytes) -> Result<Self, Vec<StructuredError>> {
    crate::init_parser();
    let buff = buff.as_ref();
    let Ok(buff_len) = i32::try_from(buff.len()) else {
      return Err(parser_error(
        bindings::xmlParserErrors_XML_SCHEMAP_FAILED_LOAD,
        format!(
          "The schema buffer has {} bytes, more than i32::MAX",
          buff.len()
        ),
      ));
    };
    if buff_len == 0 {
      return Err(parser_error(
        bindings::xmlParserErrors_XML_ERR_DOCUMENT_EMPTY,
        "The schema buffer is empty".to_string(),
      ));
    }
    let buff_bytes: Box<[u8]> = buff.into();
    let buff_ptr = buff_bytes.as_ptr() as *const c_char;
    let parser = unsafe { bindings::xmlSchemaNewMemParserCtxt(buff_ptr, buff_len) };
    Self::checked(parser, Some(buff_bytes), None)
  }

  /// Create a schema parsing context from an URL
  ///
  /// # Panics
  /// Where [`try_from_file`](Self::try_from_file) returns an error.
  pub fn from_file(path: &str) -> Self {
    Self::try_from_file(path)
      .unwrap_or_else(|errors| failed("Failed to create schema parser context from path", errors))
  }

  /// Create a schema parsing context from an URL
  ///
  /// Returns an error, where [`from_file`](Self::from_file) panics, for a path with a NUL byte, and if libxml2 makes
  /// no context. libxml2 reads the file only when the schema is parsed, by
  /// [`SchemaValidationContext::from_parser`](super::SchemaValidationContext::from_parser), which reports a missing
  /// file.
  pub fn try_from_file(path: &str) -> Result<Self, Vec<StructuredError>> {
    crate::init_parser();
    let path = common::c_path(path, bindings::xmlErrorDomain_XML_FROM_SCHEMASP)?;
    let parser = unsafe { bindings::xmlSchemaNewParserCtxt(path.as_ptr()) };
    Self::checked(parser, None, None)
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
  /// The context for `parser`, or an error if libxml2 made none: once the constructors have checked their input,
  /// only when it is out of memory.
  fn checked(
    parser: *mut bindings::_xmlSchemaParserCtxt,
    buffer: Option<Box<[u8]>>,
    document: Option<AssertUnwindSafe<Document>>,
  ) -> Result<Self, Vec<StructuredError>> {
    if parser.is_null() {
      return Err(parser_error(
        bindings::xmlParserErrors_XML_ERR_NO_MEMORY,
        "libxml2 made no schema parser context".to_string(),
      ));
    }
    Ok(Self::from_raw(parser, buffer, document))
  }

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

fn parser_error(code: bindings::xmlParserErrors, message: String) -> Vec<StructuredError> {
  common::unreported_error(bindings::xmlErrorDomain_XML_FROM_SCHEMASP, code, message)
}

/// The panic of the `from_*` constructors, with the errors their `try_from_*` returned.
#[cold]
fn failed(what: &str, errors: Vec<StructuredError>) -> ! {
  let messages: Vec<&str> = errors
    .iter()
    .filter_map(|error| error.message.as_deref())
    .collect();
  panic!("{what}: {}", messages.join("; "))
}
