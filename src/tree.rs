//! The tree functionality
use c_signatures::*;

use std::ffi::{CString, CStr};
use libc;
use libc::{c_void, c_int};
use std::hash::{Hash, Hasher};
use std::ptr;
use std::str;
use std::collections::HashSet;
use global::*;

/// An xml node
#[derive(Clone)]
pub struct Node {
  /// libxml's xmlNodePtr
  pub node_ptr: *mut c_void,
}

impl Hash for Node {
  /// Generates a hash value from the `node_ptr` value.
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.node_ptr.hash(state);
  }
}

impl PartialEq for Node {
  /// Two nodes are considered equal, if they point to the same xmlNode.
  fn eq(&self, other: &Node) -> bool {
    self.node_ptr == other.node_ptr
  }
}

impl Eq for Node {}

impl Drop for Node {
  /// Free node if it isn't bound in some document
  fn drop(&mut self) {
    // TODO: How do we drop unbound nodes?
    // unsafe {
    //   if self.node_ptr {
    //     xmlFreeNode(self.node_ptr);
    //   }
    // }
  }
}

/// A libxml2 Document
#[derive(Debug)]
pub struct Document {
  /// libxml's `DocumentPtr`
  pub doc_ptr: *mut c_void,
}


impl Drop for Document {
  ///Free document when it goes out of scope
  fn drop(&mut self) {
    unsafe {
      xmlFreeDoc(self.doc_ptr);
    }
    _libxml_global_drop();
  }
}

impl Document {
  /// Creates a new empty libxml2 document
  pub fn new() -> Result<Self, ()> {
    _libxml_global_init();
    unsafe {
      let c_version = CString::new("1.0").unwrap();
      let libxml_doc = xmlNewDoc(c_version.as_ptr());
      if libxml_doc.is_null() {
        Err(())
      } else {
        Ok(Document { doc_ptr: libxml_doc })
      }
    }
  }
  /// Creates a new `Document` from an existing libxml2 pointer
  pub fn new_ptr(doc_ptr: *mut c_void) -> Self {
    _libxml_global_init();
    Document { doc_ptr: doc_ptr }
  }
  /// Write document to `filename`
  pub fn save_file(&self, filename: &str) -> Result<c_int, ()> {
    let c_filename = CString::new(filename).unwrap();
    unsafe {
      let retval = xmlSaveFile(c_filename.as_ptr(), self.doc_ptr);
      if retval < 0 {
        return Err(());
      }
      Ok(retval)
    }
  }
  /// Get the root element of the document
  pub fn get_root_element(&self) -> Result<Node, ()> {
    unsafe {
      let node_ptr = xmlDocGetRootElement(self.doc_ptr);
      if node_ptr.is_null() {
        return Err(());
      }
      Ok(Node {
              node_ptr : node_ptr,
              // node_is_inserted : true,
          })
    }
  }

  /// Sets the root element of the document
  pub fn set_root_element(&mut self, root: &mut Node) {
    unsafe {
      xmlDocSetRootElement(self.doc_ptr, root.node_ptr);
      // root.node_is_inserted = true;
    }
  }

  /// Serializes the `Document`
  pub fn to_string(&self) -> String {
    unsafe {
      // allocate a buffer to dump into
      let mut receiver = ptr::null_mut();
      let mut size: c_int = 0;
      let c_utf8 = CString::new("UTF-8").unwrap();
      xmlDocDumpMemoryEnc(self.doc_ptr, &mut receiver, &mut size, c_utf8.as_ptr(), 1);

      let c_string = CStr::from_ptr(receiver);
      let node_string = str::from_utf8(c_string.to_bytes()).unwrap().to_owned();
      libc::free(receiver as *mut c_void);

      node_string
    }
  }

  /// Serializes a `Node` owned by this `Document
  pub fn node_to_string(&self, node: &Node) -> String {
    unsafe {
      // allocate a buffer to dump into
      let buf = xmlBufferCreate();

      // dump the node
      xmlNodeDump(buf,
                  self.doc_ptr,
                  node.node_ptr,
                  1, // level of indentation
                  0 /* disable formatting */);
      let result_ptr = xmlBufferContent(buf);
      let c_string = CStr::from_ptr(result_ptr);
      let node_string = str::from_utf8(c_string.to_bytes()).unwrap().to_owned();
      xmlBufferFree(buf);

      node_string
    }
  }

  /// Creates a node for an XML processing instruction
  pub fn create_processing_instruction(&mut self, name: &str, content: &str) -> Result<Node, ()> {
    unsafe {
      let c_name = CString::new(name).unwrap();
      let c_content = CString::new(content).unwrap();

      let node_ptr = xmlNewDocPI(self.doc_ptr, c_name.as_ptr(), c_content.as_ptr());
      if node_ptr.is_null() {
        Err(())
      } else {
        Ok(Node { node_ptr: node_ptr })
      }
    }
  }
}



// The helper functions for trees
#[inline(always)]
fn inserted_node_unless_null(ptr: *mut c_void) -> Option<Node> {
  if ptr.is_null() {
    return None;
  }
  Some(Node {
        node_ptr : ptr,
        // node_is_inserted : true,
    })
}

/// Types of xml nodes
#[derive(PartialEq)]
#[allow(missing_docs)]
pub enum NodeType {
  ElementNode,
  AttributeNode,
  TextNode,
  CDataSectionNode,
  EntityRefNode,
  EntityNode,
  PiNode,
  CommentNode,
  DocumentNode,
  DocumentTypeNode,
  DocumentFragNode,
  NotationNode,
  HtmlDocumentNode,
  DTDNode,
  ElementDecl,
  AttributeDecl,
  EntityDecl,
  NamespaceDecl,
  XIncludeStart,
  XIncludeEnd,
  DOCBDocumentNode,
}

impl NodeType {
  /// converts an integer from libxml's `enum NodeType`
  /// to an instance of our `NodeType`
  pub fn from_c_int(i: c_int) -> Option<NodeType> {
    match i {
      1 => Some(NodeType::ElementNode),
      2 => Some(NodeType::AttributeNode),
      3 => Some(NodeType::TextNode),
      4 => Some(NodeType::CDataSectionNode),
      5 => Some(NodeType::EntityRefNode),
      6 => Some(NodeType::EntityNode),
      7 => Some(NodeType::PiNode),
      8 => Some(NodeType::CommentNode),
      9 => Some(NodeType::DocumentNode),
      10 => Some(NodeType::DocumentTypeNode),
      11 => Some(NodeType::DocumentFragNode),
      12 => Some(NodeType::NotationNode),
      13 => Some(NodeType::HtmlDocumentNode),
      14 => Some(NodeType::DTDNode),
      15 => Some(NodeType::ElementDecl),
      16 => Some(NodeType::AttributeDecl),
      17 => Some(NodeType::EntityDecl),
      18 => Some(NodeType::NamespaceDecl),
      19 => Some(NodeType::XIncludeStart),
      20 => Some(NodeType::XIncludeEnd),
      21 => Some(NodeType::DOCBDocumentNode),
      _ => None,
    }
  }
}

impl Node {
  /// Create a new node, bound to a given document.
  pub fn new(name: &str, ns: Option<Namespace>, doc: &Document) -> Result<Self, ()> {
    // We will only allow to work with document-bound nodes for now, to avoid the problems of memory management.

    let c_name = CString::new(name).unwrap();
    let ns_ptr = match ns {
      None => ptr::null_mut(),
      Some(ns) => ns.ns_ptr,
    };
    unsafe {
      let node = xmlNewDocNode(doc.doc_ptr, ns_ptr, c_name.as_ptr(), ptr::null());
      if node.is_null() {
        Err(())
      } else {
        Ok(Node { node_ptr: node })//node_is_inserted : true
      }
    }
  }

  /// Returns the next sibling if it exists
  pub fn get_next_sibling(&self) -> Option<Node> {
    let ptr = unsafe { xmlNextSibling(self.node_ptr) };
    inserted_node_unless_null(ptr)
  }

  /// Returns the previous sibling if it exists
  pub fn get_prev_sibling(&self) -> Option<Node> {
    let ptr = unsafe { xmlPrevSibling(self.node_ptr) };
    inserted_node_unless_null(ptr)
  }

  /// Returns the first child if it exists
  pub fn get_first_child(&self) -> Option<Node> {
    let ptr = unsafe { xmlGetFirstChild(self.node_ptr) };
    inserted_node_unless_null(ptr)
  }

  /// Returns the parent if it exists
  pub fn get_parent(&self) -> Option<Node> {
    let ptr = unsafe { xmlGetParent(self.node_ptr) };
    inserted_node_unless_null(ptr)
  }

  /// Get the node type
  pub fn get_type(&self) -> Option<NodeType> {
    NodeType::from_c_int(unsafe { xmlGetNodeType(self.node_ptr) })
  }

  /// Add a previous sibling
  pub fn add_prev_sibling(&self, new_sibling: Node) -> Option<Node> {
    // TODO: Think of using a Result type, the libxml2 call returns NULL on error, or the child node on success
    unsafe {
      if xmlAddPrevSibling(self.node_ptr, new_sibling.node_ptr).is_null() {
        None
      } else {
        Some(new_sibling)
      }
    }
  }

  /// Add a next sibling
  pub fn add_next_sibling(&self, new_sibling: Node) -> Option<Node> {
    // TODO: Think of using a Result type, the libxml2 call returns NULL on error, or the child node on success
    unsafe {
      if xmlAddNextSibling(self.node_ptr, new_sibling.node_ptr).is_null() {
        None
      } else {
        Some(new_sibling)
      }
    }
  }

  /// Returns true iff it is a text node
  pub fn is_text_node(&self) -> bool {
    match self.get_type() {
      Some(NodeType::TextNode) => true,
      _ => false,
    }
  }

  /// Returns the name of the node (empty string if name pointer is `NULL`)
  pub fn get_name(&self) -> String {
    let name_ptr = unsafe { xmlNodeGetName(self.node_ptr) };
    if name_ptr.is_null() {
      return String::new();
    }  //empty string
    let c_string = unsafe { CStr::from_ptr(name_ptr) };
    str::from_utf8(c_string.to_bytes()).unwrap().to_owned()
  }

  /// Returns the content of the node
  /// (empty string if content pointer is `NULL`)
  pub fn get_content(&self) -> String {
    let content_ptr = unsafe { xmlNodeGetContentPointer(self.node_ptr) };
    if content_ptr.is_null() {
      return String::new();
    }  //empty string
    let c_string = unsafe { CStr::from_ptr(content_ptr) };
    str::from_utf8(c_string.to_bytes()).unwrap().to_owned()
  }

  /// Sets the text content of this `Node`
  pub fn set_content(&self, content: &str) {
    let c_content = CString::new(content).unwrap();
    unsafe { xmlNodeSetContent(self.node_ptr, c_content.as_ptr()) }
  }

  /// Returns the value of property `name`
  pub fn get_property(&self, name: &str) -> Option<String> {
    let c_name = CString::new(name).unwrap();
    let value_ptr = unsafe { xmlGetProp(self.node_ptr, c_name.as_ptr()) };
    if value_ptr.is_null() {
      return None;
    }
    let c_value_string = unsafe { CStr::from_ptr(value_ptr) };
    let prop_str = str::from_utf8(c_value_string.to_bytes()).unwrap().clone().to_owned();
    unsafe {
      libc::free(value_ptr as *mut c_void);
    }
    Some(prop_str)
  }

  /// Get a set of class names from this node's attributes
  pub fn get_class_names(&self) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(value) = self.get_property("class") {
      for n in value.split(' ') {
        set.insert(n.to_owned());
      }
    }
    set
  }

  /// Creates a new `Node` as child to the self `Node`
  pub fn add_child(&mut self, child: Node) -> Result<Node, ()> {
    unsafe {
      if xmlAddChild(self.node_ptr, child.node_ptr).is_null() {
        Err(())
      } else {
        Ok(child)
      }
    }
  }

  /// Creates a new `Node` as child to the self `Node`
  pub fn new_child(&mut self, ns: Option<Namespace>, name: &str) -> Result<Node, ()> {
    let c_name = CString::new(name).unwrap();
    let ns_ptr = match ns {
      None => ptr::null_mut(),
      Some(ns) => ns.ns_ptr,
    };
    unsafe {
      let new_ptr = xmlNewChild(self.node_ptr, ns_ptr, c_name.as_ptr(), ptr::null());
      return Ok(Node { node_ptr: new_ptr });
    }
  }

  /// Adds a new text child, to this `Node`
  pub fn add_text_child(&mut self, ns: Option<Namespace>, name: &str, content: &str) -> Result<Node, ()> {
    let c_name = CString::new(name).unwrap();
    let c_content = CString::new(content).unwrap();
    let ns_ptr = match ns {
      None => ptr::null_mut(),
      Some(ns) => ns.ns_ptr,
    };
    unsafe {
      let new_ptr = xmlNewTextChild(self.node_ptr, ns_ptr, c_name.as_ptr(), c_content.as_ptr());
      return Ok(Node { node_ptr: new_ptr });
    }
  }

  /// Append text to this `Node`
  pub fn append_text(&mut self, content: &str) -> Result<Node, ()> {
    let c_content = CString::new(content).unwrap();
    unsafe {
      let new_ptr = xmlNewText(self.node_ptr, c_content.as_ptr());
      return Ok(Node { node_ptr: new_ptr });
    }
  }
}

///An xml namespace
#[derive(Clone)]
pub struct Namespace {
  ///libxml's xmlNsPtr
  pub ns_ptr: *mut c_void,
}

impl Namespace {
  /// Creates a new namespace
  pub fn new(node: &Node, href: &str, prefix: &str) -> Result<Self, ()> {
    let c_href = CString::new(href).unwrap();
    let c_prefix = CString::new(prefix).unwrap();
    unsafe {
      let ns = xmlNewNs(node.node_ptr, c_href.as_ptr(), c_prefix.as_ptr());
      if ns.is_null() {
        Err(())
      } else {
        Ok(Namespace { ns_ptr: ns })
      }
    }
  }
}

impl Drop for Namespace {
  ///Free namespace
  fn drop(&mut self) {
    // unsafe {
    // xmlFreeNs(self.ns_ptr);
    // }
  }
}
