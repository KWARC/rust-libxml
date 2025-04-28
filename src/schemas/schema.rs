//!
//! Wrapping of the Schema (xmlSchema)
//!
use std::sync::Mutex;

use super::SchemaParserContext;

use crate::bindings;

use crate::error::StructuredError;

static SCHEMA_MUTEX: Mutex<()> = Mutex::new(());

/// Wrapper on xmlSchema
pub struct Schema(*mut bindings::_xmlSchema);

impl Schema {
  /// Create schema by having a SchemaParserContext do the actual parsing of the schema it was provided
  pub fn from_parser(parser: &mut SchemaParserContext) -> Result<Self, Vec<StructuredError>> {
    // Wrap the schema initialization with a mutex.
    // We need to do this because `xmlSchemaParse` is NOT thread-safe.
    // It isn't thread-safe because `xmlSchemaParse` calls `xmlSchemaInitTypes`
    // which manipulates shared memory in a thread-unsafe way.
    let lr = SCHEMA_MUTEX.lock().unwrap();
    
    let raw = unsafe { bindings::xmlSchemaParse(parser.as_ptr()) };
    
    drop(lr);
    if raw.is_null() {
      Err(parser.drain_errors())
    } else {
      Ok(Self(raw))
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
