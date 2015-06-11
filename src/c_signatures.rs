//! The signatures of the c functions we'll call

use libc::{c_char, c_void, c_int};

#[link(name = "xml2")]
extern "C" {
    //tree
    pub fn xmlSaveFile(filename: *const c_char, cur: *mut c_void) -> c_int;
    pub fn xmlFreeDoc(cur: *mut c_void);
    pub fn xmlFreeNode(cur: *mut c_void);
    pub fn xmlDocGetRootElement(doc: *const c_void) -> *mut c_void;

    //helper for tree
    pub fn xmlNextSibling(cur: *const c_void) -> *mut c_void;
    pub fn xmlPrevSibling(cur: *const c_void) -> *mut c_void;
    pub fn xmlGetFirstChild(cur: *const c_void) -> *mut c_void;
    pub fn xmlNodeGetName(cur: *const c_void) -> *const c_char;
    pub fn xmlNodeGetContentPointer(cur: *const c_void) -> *const c_char;
    pub fn xmlIsTextNode(cur: *const c_void) -> c_int;

    //parser
    pub fn xmlParseFile(filename: *const c_char) -> *mut c_void;
    pub fn xmlCleanupParser();

    //xpath
    pub fn xmlXPathFreeContext(ctxt: *mut c_void);
    pub fn xmlXPathNewContext(doc: *mut c_void) -> *mut c_void;
}

