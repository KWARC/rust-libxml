#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use crate::bindings::*;
use libc::{c_char, c_int, size_t};
use std::os::raw::c_void;
use std::ptr;
use std::slice;
// error handling functions
// pub fn xmlSetGenericErrorFunc(ctx: *mut c_void, handler: *mut c_void);
// pub fn xmlThrDefSetGenericErrorFunc(ctx: *mut c_void, handler: *mut c_void);

// Taken from Nokogiri (https://github.com/sparklemotion/nokogiri/blob/24bb843327306d2d71e4b2dc337c1e327cbf4516/ext/nokogiri/xml_document.c#L64)
pub fn xmlNodeRecursivelyRemoveNs(node: xmlNodePtr) {
  unsafe {
    let mut property: xmlAttrPtr;

    xmlSetNs(node, ptr::null_mut());
    let mut child: xmlNodePtr = (*node).children;
    while !child.is_null() {
      xmlNodeRecursivelyRemoveNs(child);
      child = (*child).next;
    }

    if (((*node).type_ == xmlElementType_XML_ELEMENT_NODE)
      || ((*node).type_ == xmlElementType_XML_XINCLUDE_START)
      || ((*node).type_ == xmlElementType_XML_XINCLUDE_END))
      && !(*node).nsDef.is_null()
    {
      xmlFreeNsList((*node).nsDef);
      (*node).nsDef = ptr::null_mut();
    }

    if (*node).type_ == xmlElementType_XML_ELEMENT_NODE && !(*node).properties.is_null() {
      property = (*node).properties;
      while !property.is_null() {
        if !(*property).ns.is_null() {
          (*property).ns = ptr::null_mut();
        }
        property = (*property).next;
      }
    }
  }
}
pub fn xmlGetDoc(cur: xmlNodePtr) -> xmlDocPtr {
  unsafe { (*cur).doc }
}
pub fn xmlNextNsSibling(ns: xmlNsPtr) -> xmlNsPtr {
  unsafe { (*ns).next }
}
pub fn xmlNsPrefix(ns: xmlNsPtr) -> *const c_char {
  unsafe { (*ns).prefix as *const c_char }
}
pub fn xmlNsHref(ns: xmlNsPtr) -> *const c_char {
  unsafe { (*ns).href as *const c_char }
}
pub fn xmlNodeNsDeclarations(cur: xmlNodePtr) -> xmlNsPtr {
  unsafe { (*cur).nsDef }
}
pub fn xmlNodeNs(cur: xmlNodePtr) -> xmlNsPtr {
  unsafe { (*cur).ns }
}

pub fn xmlNextPropertySibling(attr: xmlAttrPtr) -> xmlAttrPtr {
  unsafe { (*attr).next }
}
pub fn xmlAttrName(attr: xmlAttrPtr) -> *const c_char {
  unsafe { (*attr).name as *const c_char }
}
pub fn xmlAttrNs(attr: xmlAttrPtr) -> xmlNsPtr {
  unsafe { (*attr).ns }
}
pub fn xmlGetFirstProperty(node: xmlNodePtr) -> xmlAttrPtr {
  unsafe { (*node).properties }
}
pub fn xmlGetNodeType(cur: xmlNodePtr) -> xmlElementType {
  unsafe { (*cur).type_ }
}

pub fn xmlGetParent(cur: xmlNodePtr) -> xmlNodePtr {
  unsafe { (*cur).parent }
}
pub fn xmlGetFirstChild(cur: xmlNodePtr) -> xmlNodePtr {
  unsafe { (*cur).children }
}
pub fn xmlPrevSibling(cur: xmlNodePtr) -> xmlNodePtr {
  unsafe { (*cur).prev }
}

// helper for tree
pub fn xmlNextSibling(cur: xmlNodePtr) -> xmlNodePtr {
  unsafe { (*cur).next }
}

pub fn xmlNodeGetName(cur: xmlNodePtr) -> *const c_char {
  unsafe { (*cur).name as *const c_char }
}

// dummy function: no debug output at all
#[cfg(libxml_older_than_2_12)]
unsafe extern "C" fn _ignoreInvalidTagsErrorFunc(_user_data: *mut c_void, error: xmlErrorPtr) {
  unsafe {
    if !error.is_null() && (*error).code as xmlParserErrors == xmlParserErrors_XML_HTML_UNKNOWN_TAG {
      // do not record invalid, in fact (out of despair) claim we ARE well-formed, when a tag is invalid.
      HACKY_WELL_FORMED = true;
    }
  }
}
#[cfg(not(libxml_older_than_2_12))]
unsafe extern "C" fn _ignoreInvalidTagsErrorFunc(_user_data: *mut c_void, error: *const xmlError) {
  unsafe {
    if !error.is_null() && (*error).code as xmlParserErrors == xmlParserErrors_XML_HTML_UNKNOWN_TAG {
      // do not record invalid, in fact (out of despair) claim we ARE well-formed, when a tag is invalid.
      HACKY_WELL_FORMED = true;
    }
  }
}

pub fn setWellFormednessHandler(ctxt: *mut xmlParserCtxt) {
  unsafe {
    HACKY_WELL_FORMED = false;
    xmlSetStructuredErrorFunc(ctxt as *mut c_void, Some(_ignoreInvalidTagsErrorFunc));
  }
}
// helper for parser
pub fn htmlWellFormed(ctxt: *mut xmlParserCtxt) -> bool {
  unsafe { (!ctxt.is_null() && (*ctxt).wellFormed > 0) || HACKY_WELL_FORMED }
}

// helper for xpath
pub fn xmlXPathObjectNumberOfNodes(val: xmlXPathObjectPtr) -> c_int {
  unsafe {
    if val.is_null() {
      -1
    } else if (*val).nodesetval.is_null() {
      -2
    } else {
      (*(*val).nodesetval).nodeNr
    }
  }
}

pub fn xmlXPathObjectGetNodes(val: xmlXPathObjectPtr, size: size_t) -> Vec<xmlNodePtr> {
  unsafe { slice::from_raw_parts((*(*val).nodesetval).nodeTab, size).to_vec() }
}
