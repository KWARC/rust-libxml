//! The XPath functionality

use c_signatures::*;
use libc::{c_void, size_t};
use tree::{Document, Node};
use std::ffi::{CString};

///The xpath context
#[derive(Clone)]
pub struct Context {
    ///libxml's `ContextPtr`
    pub context_ptr : *mut c_void,
}


impl Drop for Context {
    ///free xpath context when it goes out of scope
    fn drop(&mut self) {
        unsafe {
            xmlXPathFreeContext(self.context_ptr);
        }
    }
}

///Essentially, the result of the evaluation of some xpath expression
#[derive(Clone)]
pub struct Object {
    ///libxml's `ObjectPtr`
    pub ptr : *mut c_void,
}


impl Context {
    ///create the xpath context for a document
    pub fn new(doc : &Document) -> Result<Context, ()> {
        let ctxtptr : *mut c_void = unsafe {
            xmlXPathNewContext(doc.doc_ptr) };
        if ctxtptr.is_null() {
            Err(())
        } else {
            Ok(Context {context_ptr : ctxtptr })
        }
    }
    ///evaluate an xpath
    pub fn evaluate(&self, xpath: &str) -> Result<Object, ()> {
        let c_xpath = CString::new(xpath).unwrap();
        let result = unsafe { xmlXPathEvalExpression(c_xpath.as_ptr(), self.context_ptr) };
        if result.is_null() {
            Err(())
        } else {
            Ok(Object {ptr : result})
        }
    }
}

impl Drop for Object {
    /// free the memory allocated
    fn drop(&mut self) {
        unsafe {
            xmlFreeXPathObject(self.ptr);
        }
    }
}

impl Object {
    ///get the number of nodes in the result set
    pub fn get_number_of_nodes(&self) -> usize {
        let v = unsafe { xmlXPathObjectNumberOfNodes(self.ptr) };
        if v == -1 {
            panic!("rust-libxml: xpath: Passed in null pointer!");
        }
        if v == -2 {
            // No nodes found!
            return 0;
        }
        if v < -2 {
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
            vec.push(Node { node_ptr : ptr, });//node_is_inserted : true
        }
        vec
    }
}
