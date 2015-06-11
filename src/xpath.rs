//! The XPath functionality

use c_signatures::*;
use libc::c_void;
use tree::XmlDoc;

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


impl XmlXPathContext {
    ///create xpath context for a document
    pub fn new(doc : &XmlDoc) -> Result<XmlXPathContext, ()> {
        let ctxtptr : *mut c_void = unsafe {
            xmlXPathNewContext(doc.doc_ptr) };
        if ctxtptr.is_null() {
            Err(())
        } else {
            Ok(XmlXPathContext {context_ptr : ctxtptr })
        }
    }
}
