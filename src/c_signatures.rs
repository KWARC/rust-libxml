//! The signatures of the c functions we'll call

use libc::{c_char, c_void, c_int};

#[link(name = "xml2")]
extern "C" {
    //tree
    pub fn xmlSaveFile(filename: *const c_char, cur: *mut c_void) -> c_int;
    pub fn xmlFreeDoc(cur: *mut c_void);
    pub fn xmlDocGetRootElement(doc: *const c_void) -> *mut c_void;

    //parser
    pub fn xmlParseFile(filename: *const c_char) -> *mut c_void;
    pub fn xmlCleanupParser();
}

