//! The tree functionality

use c_signatures::*;

use std::ffi::CString;
use libc::{c_void, c_int};

///An xml node
pub struct XmlNodeRef {
    ///libxml's xmlNodePtr
    pub xml_node_ptr : *mut c_void,
    ///The node is inserted into a document.
    ///*Warning*: This isn't 100% safe if you have several references
    ///to a node and unlink one of them. So please be reasonable.
    pub node_is_inserted : bool,
}

///An xml document
pub struct XmlDoc {
    ///libxml's xmlDocPtr
    pub xml_doc_ptr : *mut c_void,  //Can we change the visibility somehow?
}


impl Drop for XmlDoc {
    ///Free document when it goes out of scope
    fn drop(&mut self) {
        unsafe {
            xmlFreeDoc(self.xml_doc_ptr);
        }
    }
}


impl XmlDoc {
    ///Write document to `filename`
    pub fn save_file(&self, filename : &str) -> Result<c_int, ()> {
        let c_filename = CString::new(filename).unwrap().as_ptr();
        unsafe {
            let retval = xmlSaveFile(c_filename, self.xml_doc_ptr);
            if retval < 0 {
                return Err(());
            }
            Ok(retval)
        }
    }
    ///Get the root element of the document
    pub fn get_root_element(&self) -> Result<XmlNodeRef, ()> {
        unsafe {
            let node_ptr = xmlDocGetRootElement(self.xml_doc_ptr);
            if node_ptr.is_null() {
                return Err(());
            }
            Ok(XmlNodeRef {
                xml_node_ptr : node_ptr,
                node_is_inserted : true,
            })
        }
    }
}
