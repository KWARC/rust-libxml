//! The XPath functionality

use c_signatures::*;
use libc::{c_void, size_t};
use tree::{Document, Node};
use std::ffi::{CString};

///The xpath context
#[allow(raw_pointer_derive)]
#[derive(Clone)]
pub struct XmlXPathContext {
    ///libxml's `xmlXPathContextPtr`
    pub context_ptr : *mut c_void,
}


impl Drop for XmlXPathContext {
    ///free xpath context when it goes out of scope
    fn drop(&mut self) {
        unsafe {
            xmlXPathFreeContext(self.context_ptr);
        }
    }
}

///Essentially, the result of the evaluation of some xpath expression
#[allow(raw_pointer_derive)]
#[derive(Clone)]
pub struct XmlXPathObject {
    ///libxml's `xmlXpathObjectPtr`
    pub ptr : *mut c_void,
}


impl XmlXPathContext {
    ///create the xpath context for a document
    pub fn new(doc : &Document) -> Result<XmlXPathContext, ()> {
        let ctxtptr : *mut c_void = unsafe {
            xmlXPathNewContext(doc.doc_ptr) };
        if ctxtptr.is_null() {
            Err(())
        } else {
            Ok(XmlXPathContext {context_ptr : ctxtptr })
        }
    }
    ///evaluate an xpath
    pub fn evaluate(&self, xpath: &str) -> Result<XmlXPathObject, ()> {
        let c_xpath = CString::new(xpath).unwrap().as_ptr();
        let result = unsafe { xmlXPathEvalExpression(c_xpath, self.context_ptr) };
        if result.is_null() {
            Err(())
        } else {
            Ok(XmlXPathObject {ptr : result})
        }
    }
}

impl Drop for XmlXPathObject {
    /// free the memory allocated
    fn drop(&mut self) {
        unsafe {
            xmlFreeXPathObject(self.ptr);
        }
    }
}

impl XmlXPathObject {
    ///get the number of nodes in the result set
    pub fn get_number_of_nodes(&self) -> usize {
        let v = unsafe { xmlXPathObjectNumberOfNodes(self.ptr) };
        if v < 0 {
            panic!("rust-libxml: xpath: expected non-negative number of result nodes");
        }
        v as usize
    }

    /// returns the result set as a vector of node references
    pub fn get_nodes_as_vec(&self) -> Vec<Node> {
        let n = self.get_number_of_nodes();
        let mut vec : Vec<Node> = Vec::with_capacity(n);
        for i in 0..n {
            let ptr : *mut c_void = unsafe {
                xmlXPathObjectGetNode(self.ptr, i as size_t) };
            if ptr.is_null() {
                panic!("rust-libxml: xpath: found null pointer result set");
            }
            vec.push(Node { node_ptr : ptr, node_is_inserted : true });
        }
        vec
    }
}
