//!
//! Wrapping of the Validation Context (xmlSchemaValidCtxt)
//!
use super::common;

use super::Schema;
use super::SchemaParserContext;

use crate::bindings;

use crate::tree::node::Node;
use crate::tree::document::Document;

use crate::error::XmlStructuredError;

use std::rc::Rc;
use std::ffi::CString;
use std::cell::RefCell;


/// Wrapper on xmlSchemaValidCtxt
pub struct SchemaValidationContext
{
    ctxt:   *mut bindings::_xmlSchemaValidCtxt,
    errlog: Rc<RefCell<Vec<XmlStructuredError>>>,

    _schema: Schema,
}


impl SchemaValidationContext
{
    /// Create a schema validation context from a parser object
    pub fn from_parser(parser: &mut SchemaParserContext) -> Result<Self, Vec<XmlStructuredError>>
    {
        let schema = Schema::from_parser(parser);

        match schema
        {
            Ok(s) => {
                let ctx = unsafe { bindings::xmlSchemaNewValidCtxt(s.as_ptr()) };

                if ctx.is_null() {
                    panic!("Failed to create validation context from XML schema")   // TODO error handling
                }

                Ok(Self::from_raw(ctx, s))
            }

            Err(e) => Err(e)
        }
    }

    /// Validates a given Document, that is to be tested to comply with the loaded XSD schema definition
    pub fn validate_document(&mut self, doc: &Document) -> Result<(), Vec<XmlStructuredError>>
    {
        let rc = unsafe { bindings::xmlSchemaValidateDoc(self.ctxt, doc.doc_ptr()) };

        match rc
        {
            -1 => panic!("Failed to validate document due to internal error"),   // TODO error handling
             0 => Ok(()),
             _ => Err(self.drain_errors())
        }
    }

    /// Validates a given file from path for its compliance with the loaded XSD schema definition
    pub fn validate_file(&mut self, path: &str) -> Result<(), Vec<XmlStructuredError>>
    {
        let path     = CString::new(path).unwrap();  // TODO error handling for \0 containing strings
        let path_ptr = path.as_bytes_with_nul().as_ptr() as *const i8;

        let rc = unsafe { bindings::xmlSchemaValidateFile(self.ctxt, path_ptr, 0) };

        match rc
        {
            -1 => panic!("Failed to validate file due to internal error"),   // TODO error handling
             0 => Ok(()),
             _ => Err(self.drain_errors())
        }
    }

    /// Validates a branch or leaf of a document given as a Node against the loaded XSD schema definition
    pub fn validate_node(&mut self, node: &Node) -> Result<(), Vec<XmlStructuredError>>
    {
        let rc = unsafe { bindings::xmlSchemaValidateOneElement(self.ctxt, node.node_ptr()) };

        match rc
        {
            -1 => panic!("Failed to validate element due to internal error"),   // TODO error handling
             0 => Ok(()),
             _ => Err(self.drain_errors())
        }
    }

    /// Drains error log from errors that might have accumulated while validating something
    pub fn drain_errors(&mut self) -> Vec<XmlStructuredError>
    {
        self.errlog.borrow_mut()
            .drain(0..)
            .collect()
    }

    /// Return a raw pointer to the underlying xmlSchemaValidCtxt structure
    pub fn as_ptr(&self) -> *mut bindings::_xmlSchemaValidCtxt
    {
        self.ctxt
    }
}


/// Private Interface
impl SchemaValidationContext
{
    fn from_raw(ctx: *mut bindings::_xmlSchemaValidCtxt, schema: Schema) -> Self
    {
        let errors = Rc::new(RefCell::new(Vec::new()));

        unsafe {
            bindings::xmlSchemaSetValidStructuredErrors(
                ctx,
                Some(common::structured_error_handler),
                Box::into_raw(Box::new(Rc::downgrade(&errors))) as *mut _
            );
        }

        Self {ctxt: ctx, errlog: errors, _schema: schema}
    }
}


impl Drop for SchemaValidationContext
{
    fn drop(&mut self)
    {
        unsafe { bindings::xmlSchemaFreeValidCtxt(self.ctxt) }
    }
}
