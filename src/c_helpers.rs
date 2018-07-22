use bindings::*;
use libc::{c_char, c_int, size_t};
// error handling functions
// pub fn xmlSetGenericErrorFunc(ctx: *mut c_void, handler: *mut c_void);
// pub fn xmlThrDefSetGenericErrorFunc(ctx: *mut c_void, handler: *mut c_void);
extern "C" {
  pub fn xmlNodeRecursivelyRemoveNs(node: xmlNodePtr);
  pub fn xmlGetDoc(cur: xmlNodePtr) -> xmlDocPtr;
  pub fn xmlNextNsSibling(attr: xmlNsPtr) -> xmlNsPtr;
  pub fn xmlNsPrefix(ns: xmlNsPtr) -> *const c_char;
  pub fn xmlNsHref(ns: xmlNsPtr) -> *const c_char;
  pub fn xmlNodeNsDeclarations(cur: xmlNodePtr) -> xmlNsPtr;
  pub fn xmlNodeNs(cur: xmlNodePtr) -> xmlNsPtr;

  pub fn xmlNextPropertySibling(attr: xmlAttrPtr) -> xmlAttrPtr;
  pub fn xmlAttrName(attr: xmlAttrPtr) -> *const c_char;
  pub fn xmlGetFirstProperty(node: xmlNodePtr) -> xmlAttrPtr;
  pub fn xmlGetNodeType(cur: xmlNodePtr) -> c_int;
  pub fn xmlGetParent(cur: xmlNodePtr) -> xmlNodePtr;
  pub fn xmlGetFirstChild(cur: xmlNodePtr) -> xmlNodePtr;
  pub fn xmlPrevSibling(cur: xmlNodePtr) -> xmlNodePtr;

  // helper for tree
  pub fn xmlNextSibling(cur: xmlNodePtr) -> xmlNodePtr;
  pub fn xmlNodeGetName(cur: xmlNodePtr) -> *const c_char;

  pub fn setIndentTreeOutput(indent: c_int);
  pub fn getIndentTreeOutput() -> c_int;
  pub fn setWellFormednessHandler(ctxt: *mut xmlParserCtxt);
  // helper for parser
  pub fn htmlWellFormed(ctxt: *mut xmlParserCtxt) -> c_int;

  // helper for xpath
  pub fn xmlXPathObjectNumberOfNodes(val: xmlXPathObjectPtr) -> c_int;
  pub fn xmlXPathObjectGetNode(val: xmlXPathObjectPtr, index: size_t) -> xmlNodePtr;

}
