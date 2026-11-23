//!
//! Wrapping of the Validation Context (xmlSchemaValidCtxt)
//!
use super::common;

use super::Schema;
use super::SchemaParserContext;

use crate::bindings;

use crate::tree::document::Document;
use crate::tree::node::Node;

use crate::error::StructuredError;

/// Wrapper on xmlSchemaValidCtxt
pub struct SchemaValidationContext {
  ctxt: *mut bindings::_xmlSchemaValidCtxt,
  errlog: *mut Vec<StructuredError>,
  _schema: Schema,
}

impl SchemaValidationContext {
  /// Create a schema validation context from a parser object
  ///
  /// Returns the errors of parsing the schema, or an error if libxml2 makes no context (out of memory).
  pub fn from_parser(parser: &mut SchemaParserContext) -> Result<Self, Vec<StructuredError>> {
    let schema = Schema::from_parser(parser)?;
    let ctx = unsafe { bindings::xmlSchemaNewValidCtxt(schema.as_ptr()) };
    if ctx.is_null() {
      return Err(common::unreported_error(
        bindings::xmlErrorDomain_XML_FROM_SCHEMASV,
        bindings::xmlParserErrors_XML_ERR_NO_MEMORY,
        "libxml2 made no schema validation context".to_string(),
      ));
    }
    Ok(Self::from_raw(ctx, schema))
  }

  /// Validates a given Document, that is to be tested to comply with the loaded XSD schema definition
  pub fn validate_document(&mut self, doc: &Document) -> Result<(), Vec<StructuredError>> {
    let rc = unsafe { bindings::xmlSchemaValidateDoc(self.ctxt, doc.doc_ptr()) };

    self.validation_result(rc)
  }

  /// Validates a given file from path for its compliance with the loaded XSD schema definition
  ///
  /// A path with a NUL byte is an error (before 0.3.26, a panic).
  pub fn validate_file(&mut self, path: &str) -> Result<(), Vec<StructuredError>> {
    let path = common::c_path(path, bindings::xmlErrorDomain_XML_FROM_SCHEMASV)?;
    let rc = unsafe { bindings::xmlSchemaValidateFile(self.ctxt, path.as_ptr(), 0) };

    self.validation_result(rc)
  }

  /// Validates a branch or leaf of a document given as a Node against the loaded XSD schema definition
  pub fn validate_node(&mut self, node: &Node) -> Result<(), Vec<StructuredError>> {
    let rc = unsafe { bindings::xmlSchemaValidateOneElement(self.ctxt, node.node_ptr()) };

    self.validation_result(rc)
  }

  fn validation_result(&mut self, rc: i32) -> Result<(), Vec<StructuredError>> {
    if rc == 0 {
      return Ok(());
    }
    let errors = self.drain_errors();
    if errors.is_empty() {
      return Err(common::unreported_error(
        bindings::xmlErrorDomain_XML_FROM_SCHEMASV,
        bindings::xmlParserErrors_XML_SCHEMAV_INTERNAL,
        format!("Schema validation failed with status {rc} without a diagnostic"),
      ));
    }
    Err(errors)
  }

  /// Drains error log from errors that might have accumulated while validating something
  pub fn drain_errors(&mut self) -> Vec<StructuredError> {
    assert!(!self.errlog.is_null());
    let errors = unsafe { &mut *self.errlog };
    std::mem::take(errors)
  }

  /// Return a raw pointer to the underlying xmlSchemaValidCtxt structure
  pub fn as_ptr(&self) -> *mut bindings::_xmlSchemaValidCtxt {
    self.ctxt
  }
}

/// Private Interface
impl SchemaValidationContext {
  fn from_raw(ctx: *mut bindings::_xmlSchemaValidCtxt, schema: Schema) -> Self {
    let errlog = Box::into_raw(Box::<Vec<StructuredError>>::default());

    unsafe {
      bindings::xmlSchemaSetValidStructuredErrors(
        ctx,
        Some(common::structured_error_handler),
        errlog as *mut _,
      );
    }

    Self {
      ctxt: ctx,
      errlog,
      _schema: schema,
    }
  }
}

impl Drop for SchemaValidationContext {
  fn drop(&mut self) {
    unsafe {
      bindings::xmlSchemaFreeValidCtxt(self.ctxt);
      if !self.errlog.is_null() {
        drop(Box::from_raw(self.errlog));
      }
    }
  }
}
