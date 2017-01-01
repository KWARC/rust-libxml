//! The signatures of the c functions we'll call

use libc::{c_char, c_void, c_int, c_uint, size_t};

#[link(name = "xml2")]
extern "C" {
  // tree
  pub fn xmlSaveFile(filename: *const c_char, cur: *mut c_void) -> c_int;
  pub fn xmlNewDoc(version: *const c_char) -> *mut c_void;
  pub fn xmlFreeDoc(cur: *mut c_void);
  // pub fn xmlFree(name : *const c_char);
  // pub fn xmlNewNode(ns : *mut c_void, name: *const c_char) -> *mut c_void;
  pub fn xmlNewDocNode(doc: *mut c_void, ns: *mut c_void, name: *const c_char, content: *const c_char) -> *mut c_void;
  pub fn xmlNewDocText(doc: *mut c_void, content: *const c_char) -> *mut c_void;
  // pub fn xmlFreeNode(cur: *mut c_void);
  pub fn xmlNewNs(node: *mut c_void, href: *const c_char, prefix: *const c_char) -> *mut c_void;
  pub fn xmlNewChild(parent: *mut c_void, ns: *mut c_void, name: *const c_char, content: *const c_char) -> *mut c_void;
  pub fn xmlNewTextChild(parent: *mut c_void, ns: *mut c_void, name: *const c_char, content: *const c_char) -> *mut c_void;
  // pub fn xmlNewText(parent: *mut c_void, content: *const c_char) -> *mut c_void;
  pub fn xmlNewDocPI(doc: *mut c_void, name: *const c_char, content: *const c_char) -> *mut c_void;
  // pub fn xmlFreeNs(cur: *mut c_void);
  // pub fn xmlNewDocFragment(doc: *mut c_void) -> *mut c_void;
  pub fn xmlDocGetRootElement(doc: *const c_void) -> *mut c_void;
  pub fn xmlDocSetRootElement(doc: *const c_void, root: *const c_void) -> *mut c_void;
  pub fn xmlGetProp(node: *const c_void, name: *const c_char) -> *const c_char;
  pub fn xmlSetProp(node: *const c_void, name: *const c_char, value: *const c_char) -> *const c_char;
  pub fn xmlHasProp(node: *const c_void, name: *const c_char) -> *mut c_void;
  pub fn xmlRemoveProp(attr_node: *const c_void) -> c_int;
  pub fn xmlGetNsProp(node: *const c_void, name: *const c_char, ns: *const c_char) -> *const c_char;
  pub fn xmlGetFirstProperty(node: *const c_void) -> *mut c_void;
  pub fn xmlNextPropertySibling(attr: *const c_void) -> *mut c_void;
  pub fn xmlAttrName(attr: *const c_void) -> *const c_char;
  pub fn xmlGetNsList(doc: *const c_void, node: *const c_void) -> *const *mut c_void;
  pub fn xmlSetNs(node: *const c_void, ns: *const c_void);
  pub fn xmlSetNsProp(node: *const c_void, ns: *const c_void, name: *const c_char, value: *const c_char);
  pub fn xmlNsPrefix(ns: *const c_void) -> *const c_char;
  pub fn xmlNsURL(ns: *const c_void) -> *const c_char;
  pub fn xmlCopyNamespace(ns: *const c_void) -> *mut c_void;
  // helper for tree
  pub fn xmlTextConcat(node: *const c_void, text: *const c_char, len: c_int) -> c_int;
  pub fn xmlNextSibling(cur: *const c_void) -> *mut c_void;
  pub fn xmlPrevSibling(cur: *const c_void) -> *mut c_void;
  pub fn xmlAddChild(cur: *const c_void, new: *const c_void) -> *mut c_void;
  pub fn xmlAddPrevSibling(cur: *const c_void, new: *const c_void) -> *mut c_void;
  pub fn xmlAddNextSibling(cur: *const c_void, new: *const c_void) -> *mut c_void;
  pub fn xmlGetFirstChild(cur: *const c_void) -> *mut c_void;
  pub fn xmlGetLastChild(cur: *const c_void) -> *mut c_void;
  pub fn xmlGetParent(cur: *const c_void) -> *mut c_void;
  pub fn xmlNodeGetName(cur: *const c_void) -> *const c_char;
  pub fn xmlNodeGetContentPointer(cur: *const c_void) -> *const c_char;
  pub fn xmlNodeSetContent(node: *mut c_void, cur: *const c_char);
  pub fn xmlGetNodeType(cur: *const c_void) -> c_int;
  pub fn xmlBufferCreate() -> *mut c_void;
  pub fn xmlBufferFree(buf: *mut c_void);
  pub fn xmlBufferContent(buf: *mut c_void) -> *const c_char;
  pub fn xmlNodeDump(buf: *mut c_void, doc: *mut c_void, node: *mut c_void, indent: c_int, disable_format: c_int);
  pub fn xmlNodeNsDeclarations(cur: *const c_void) -> *mut c_void;
  pub fn xmlNodeNs(cur: *const c_void) -> *mut c_void;
  pub fn xmlNextNsSibling(attr: *const c_void) -> *mut c_void;
  // pub fn xmlDocDumpMemory(doc: *mut c_void, receiver: *mut *mut c_char, size: *const c_int, format: c_int );
  pub fn xmlDocDumpMemoryEnc(doc: *mut c_void, receiver: *mut *mut c_char, size: *const c_int, encoding: *const c_char, format: c_int);
  pub fn xmlDocDumpFormatMemoryEnc(doc: *mut c_void, receiver: *mut *mut c_char, size: *const c_int, encoding: *const c_char, format: c_int);
  pub fn setIndentTreeOutput(indent: c_int);
  pub fn getIndentTreeOutput() -> c_int;

  // parser
  pub fn xmlReadFile(filename: *const c_char, encoding: *const c_char, options: c_uint) -> *mut c_void;
  // pub fn htmlParseFile(filename: *const c_char, encoding: *const c_char) -> *mut c_void;
  pub fn htmlReadFile(filename: *const c_char, encoding: *const c_char, options: c_uint) -> *mut c_void;
  pub fn htmlReadDoc(html_string: *const c_char, url: *const c_char, encoding: *const c_char, options: c_uint) -> *mut c_void;
  pub fn xmlReadDoc(xml_string: *const c_char, url: *const c_char, encoding: *const c_char, options: c_uint) -> *mut c_void;
  // pub fn xmlParseDoc(xml_string: *const c_char) -> *mut c_void;
  // pub fn htmlParseDoc(xml_string: *const c_char, encoding: *const c_char) -> *mut c_void;
  pub fn htmlNewParserCtxt() -> *mut c_void;
  pub fn htmlCtxtReadDoc(ctxt: *mut c_void, html_string: *const c_char, url: *mut c_void, encoding: *const c_char, options: c_uint) -> *mut c_void;
  // pub fn htmlSAXParseDoc(xml_string: *const c_char, encoding: *const c_char, sax: *mut c_void, user_data: *mut c_void) -> *mut c_void;
  pub fn xmlInitParser();
  pub fn xmlCleanupParser();
  // pub fn xmlMemoryDump();
  pub fn xmlInitGlobals();
  pub fn xmlCleanupGlobals();
  // pub fn xmlFree(some: *const c_char);
  pub fn xmlKeepBlanksDefault(flag: c_uint) -> c_uint;
  // pub fn xmlFreeParserCtxt(ctxt: *mut c_void);
  pub fn htmlFreeParserCtxt(ctxt: *mut c_void);
  // helper for parser
  pub fn htmlWellFormed(ctxt: *mut c_void) -> c_int;

  // xpath
  pub fn xmlXPathFreeContext(ctxt: *mut c_void);
  pub fn xmlXPathNewContext(doc: *mut c_void) -> *mut c_void;
  pub fn xmlXPathEvalExpression(str: *const c_char, ctxt: *mut c_void) -> *mut c_void;
  pub fn xmlXPathCastToString(val: *const c_void) -> *const c_char;
  pub fn xmlXPathRegisterNs(ctxt: *mut c_void, prefix: *const c_char, href: *const c_char) -> c_int;
  // helper for xpath
  pub fn xmlXPathObjectNumberOfNodes(val: *const c_void) -> c_int;
  pub fn xmlXPathObjectGetNode(val: *const c_void, index: size_t) -> *mut c_void;
  pub fn xmlFreeXPathObject(val: *const c_void);

  // error handling functions
  // pub fn xmlSetGenericErrorFunc(ctx: *mut c_void, handler: *mut c_void);
  // pub fn xmlThrDefSetGenericErrorFunc(ctx: *mut c_void, handler: *mut c_void);
  pub fn setWellFormednessHandler(ctxt: *mut c_void);
}
