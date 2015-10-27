//! The signatures of the c functions we'll call

use libc::{c_char, c_void, c_int, c_uint, size_t};

#[link(name = "xml2")]
extern "C" {
    //tree
    pub fn xmlSaveFile(filename: *const c_char, cur: *mut c_void) -> c_int;
    pub fn xmlNewDoc(version: *const c_char) -> *mut c_void;
    pub fn xmlFreeDoc(cur: *mut c_void);
    // pub fn xmlFree(name : *const c_char);
    // pub fn xmlNewNode(ns : *mut c_void, name: *const c_char) -> *mut c_void;
    pub fn xmlNewDocNode(doc: *mut c_void, ns : *mut c_void, name: *const c_char, content: *const c_char) -> *mut c_void;
    // pub fn xmlFreeNode(cur: *mut c_void);
    pub fn xmlNewNs(node : *mut c_void, href: *const c_char, prefix: *const c_char) -> *mut c_void;
    // pub fn xmlFreeNs(cur: *mut c_void);
    // pub fn xmlNewDocFragment(doc: *mut c_void) -> *mut c_void;
    pub fn xmlDocGetRootElement(doc: *const c_void) -> *mut c_void;
    pub fn xmlDocSetRootElement(doc: *const c_void, root: *const c_void) -> *mut c_void;
    pub fn xmlGetProp(node: *const c_void, name: *const c_char) -> *const c_char;

    //helper for tree
    pub fn xmlNextSibling(cur: *const c_void) -> *mut c_void;
    pub fn xmlPrevSibling(cur: *const c_void) -> *mut c_void;
    pub fn xmlGetFirstChild(cur: *const c_void) -> *mut c_void;
    pub fn xmlNodeGetName(cur: *const c_void) -> *const c_char;
    pub fn xmlNodeGetContentPointer(cur: *const c_void) -> *const c_char;
    pub fn xmlNodeSetContent(node : *mut c_void, cur: *const c_char);
    pub fn xmlGetNodeType(cur: *const c_void) -> c_int;

    //parser
    pub fn xmlParseFile(filename: *const c_char) -> *mut c_void;
    // pub fn htmlParseFile(filename: *const c_char, encoding: *const c_char) -> *mut c_void;
    pub fn htmlReadFile(filename: *const c_char, encoding: *const c_char, options: c_uint) -> *mut c_void;
    pub fn xmlInitParser();
    pub fn xmlCleanupParser();
    // pub fn xmlMemoryDump();
    pub fn xmlInitGlobals();
    pub fn xmlCleanupGlobals();

    //xpath
    pub fn xmlXPathFreeContext(ctxt: *mut c_void);
    pub fn xmlXPathNewContext(doc: *mut c_void) -> *mut c_void;
    pub fn xmlXPathEvalExpression(str: *const c_char, ctxt: *mut c_void) -> *mut c_void;

    //helper for xpath
    pub fn xmlXPathObjectNumberOfNodes(val: *const c_void) -> c_int;
    pub fn xmlXPathObjectGetNode(val: *const c_void, index: size_t) -> *mut c_void;
    pub fn xmlFreeXPathObject(val: *const c_void);
}
