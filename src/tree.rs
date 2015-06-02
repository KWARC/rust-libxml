//! The tree functionality

use c_signatures::*;

use std::ffi::CString;
use libc::{c_void, c_int};

///An xml document
pub struct XmlDoc {
    ///It's libxml's xmlDocPtr
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
}
