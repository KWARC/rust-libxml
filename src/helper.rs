//! The helper functions, i.e. the functions which are not part of the libxml
//! interface, but which will be needed, because we abstracted away from the
//! xmlNode and xmlDoc structs.

use c_signatures::*;
use tree::*;
use std::ffi::CStr;
use std::str;
use libc::c_void;


fn inserted_node_if_not_null(ptr: *mut c_void) -> Option<XmlNodeRef> {
    if ptr.is_null() {
        return None
    }
    Some(XmlNodeRef {
        node_ptr : ptr,
        node_is_inserted : true,
    })
}

impl XmlNodeRef {
    pub fn get_next_sibling(&self) -> Option<XmlNodeRef> {
        let ptr = unsafe { xmlNextSibling(self.node_ptr) };
        inserted_node_if_not_null(ptr)
    }

    pub fn get_prev_sibling(&self) -> Option<XmlNodeRef> {
        let ptr = unsafe { xmlPrevSibling(self.node_ptr) };
        inserted_node_if_not_null(ptr)
    }

    pub fn get_first_child(&self) -> Option<XmlNodeRef> {
        let ptr = unsafe { xmlGetFirstChild(self.node_ptr) };
        inserted_node_if_not_null(ptr)
    }

    pub fn is_text_node(&self) -> bool {
        match unsafe {xmlIsTextNode(self.node_ptr)} {
            0 => false,
            1 => true,
            _ => panic!("xmlIsTextNode returned neither 0 nor 1"),
        }
    }

    pub fn get_name(&self) -> String {
        let name_ptr = unsafe { xmlNodeGetName(self.node_ptr) };
        if name_ptr.is_null() { return String::new() }  //empty string
        let c_string = unsafe { CStr::from_ptr(name_ptr) };
        str::from_utf8(c_string.to_bytes()).unwrap().to_owned()
    }

    pub fn get_content(&self) -> String {
        let content_ptr = unsafe { xmlNodeGetContentPointer(self.node_ptr) };
        if content_ptr.is_null() { return String::new() }  //empty string
        let c_string = unsafe { CStr::from_ptr(content_ptr) };
        str::from_utf8(c_string.to_bytes()).unwrap().to_owned()
   }
}
