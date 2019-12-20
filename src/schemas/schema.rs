//!
//! Wrapping of the Schema (xmlSchema)
//!
use super::SchemaParserContext;

use crate::bindings;

use crate::error::StructuredError;

/// Wrapper on xmlSchema
pub struct Schema(*mut bindings::_xmlSchema);

impl Schema {
  /// Create schema by having a SchemaParserContext do the actual parsing of the schema it was provided
  pub fn from_parser(parser: &mut SchemaParserContext) -> Result<Self, Vec<StructuredError>> {
    let raw = unsafe { bindings::xmlSchemaParse(parser.as_ptr()) };

    if raw.is_null() {
      Err(parser.drain_errors())
    } else {
      Ok(Self { 0: raw })
    }
  }

  /// Return a raw pointer to the underlying xmlSchema structure
  pub fn as_ptr(&self) -> *mut bindings::_xmlSchema {
    self.0
  }
}

impl Drop for Schema {
  fn drop(&mut self) {
    unsafe { bindings::xmlSchemaFree(self.0) }
  }
}
