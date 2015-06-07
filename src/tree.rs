//! The tree functionality

use c_signatures::*;

use std::ffi::CString;
use libc::{c_void, c_int};
use std::hash::{Hash, Hasher};

///An xml node
#[allow(raw_pointer_derive)]
#[derive(Clone, Copy)]
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

///An xml document
pub struct XmlDoc {
    ///libxml's xmlDocPtr
    pub doc_ptr : *mut c_void,  //Can we change the visibility somehow?
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
