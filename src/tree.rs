//! The tree functionality

use c_signatures::*;

use std::ffi::{CString, CStr};
use libc::{c_void, c_int};
use std::hash::{Hash, Hasher};
use std::str;

///An xml node
#[allow(raw_pointer_derive)]
#[derive(Clone)]
pub struct XmlNodeRef {
    ///libxml's xmlNodePtr
    pub node_ptr : *mut c_void,
    ///The node is inserted into a document.
    ///*Warning*: This isn't 100% safe if you have several references
    ///to a node and unlink one of them. So please be reasonable.
    pub node_is_inserted : bool,
}

impl Hash for XmlNodeRef {
    /// Generates a hash value from the `node_ptr` value.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.node_ptr.hash(state);
    }
}

impl PartialEq for XmlNodeRef {
    /// Two nodes are considered equal, if they point to the same xmlNode.
    fn eq(&self, other: &XmlNodeRef) -> bool {
        self.node_ptr == other.node_ptr
    }
}

impl Eq for XmlNodeRef { }

impl Drop for XmlNodeRef {
    ///Free node if it isn't inserted in some document
    fn drop(&mut self) {
        if !self.node_is_inserted {
            unsafe {
                xmlFreeNode(self.node_ptr);
            }
        }
    }
}

///An xml document
pub struct XmlDoc {
    ///libxml's `xmlDocPtr`
    pub doc_ptr : *mut c_void,
}


impl Drop for XmlDoc {
    ///Free document when it goes out of scope
    fn drop(&mut self) {
        unsafe {
            xmlFreeDoc(self.doc_ptr);
        }
    }
}


impl XmlDoc {
    ///Write document to `filename`
    pub fn save_file(&self, filename : &str) -> Result<c_int, ()> {
        let c_filename = CString::new(filename).unwrap().as_ptr();
        unsafe {
            let retval = xmlSaveFile(c_filename, self.doc_ptr);
            if retval < 0 {
                return Err(());
            }
            Ok(retval)
        }
    }
    ///Get the root element of the document
    pub fn get_root_element(&self) -> Result<XmlNodeRef, ()> {
        unsafe {
            let node_ptr = xmlDocGetRootElement(self.doc_ptr);
            if node_ptr.is_null() {
                return Err(());
            }
            Ok(XmlNodeRef {
                node_ptr : node_ptr,
                node_is_inserted : true,
            })
        }
    }
}




// The helper functions for trees

#[inline(always)]
fn inserted_node_unless_null(ptr: *mut c_void) -> Option<XmlNodeRef> {
    if ptr.is_null() {
        return None
    }
    Some(XmlNodeRef {
        node_ptr : ptr,
        node_is_inserted : true,
    })
}

impl XmlNodeRef {
    ///Returns the next sibling if it exists
    pub fn get_next_sibling(&self) -> Option<XmlNodeRef> {
        let ptr = unsafe { xmlNextSibling(self.node_ptr) };
        inserted_node_unless_null(ptr)
    }

    ///Returns the previous sibling if it exists
    pub fn get_prev_sibling(&self) -> Option<XmlNodeRef> {
        let ptr = unsafe { xmlPrevSibling(self.node_ptr) };
        inserted_node_unless_null(ptr)
    }

    ///Returns the first child if it exists
    pub fn get_first_child(&self) -> Option<XmlNodeRef> {
        let ptr = unsafe { xmlGetFirstChild(self.node_ptr) };
        inserted_node_unless_null(ptr)
    }

    ///Returns true iff it is a text node
    pub fn is_text_node(&self) -> bool {
        match unsafe {xmlIsTextNode(self.node_ptr)} {
            0 => false,
            1 => true,
            _ => panic!("xmlIsTextNode returned neither 0 nor 1"),
        }
    }

    ///Returns the name of the node (empty string if name pointer is `NULL`)
    pub fn get_name(&self) -> String {
        let name_ptr = unsafe { xmlNodeGetName(self.node_ptr) };
        if name_ptr.is_null() { return String::new() }  //empty string
        let c_string = unsafe { CStr::from_ptr(name_ptr) };
        str::from_utf8(c_string.to_bytes()).unwrap().to_owned()
    }

    ///Returns the content of the node
    ///(empty string if content pointer is `NULL`)
    pub fn get_content(&self) -> String {
        let content_ptr = unsafe { xmlNodeGetContentPointer(self.node_ptr) };
        if content_ptr.is_null() { return String::new() }  //empty string
        let c_string = unsafe { CStr::from_ptr(content_ptr) };
        str::from_utf8(c_string.to_bytes()).unwrap().to_owned()
   }
}

