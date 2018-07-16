//! The tree functionality
use c_signatures::*;

use libc;
use libc::{c_int, c_void};
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::hash::{Hash, Hasher};
use std::mem;
use std::ptr;
use std::str;

use std::cell::RefCell;
use std::rc::Rc;

type NodeRef = Rc<RefCell<_Node>>;

#[derive(Debug)]
struct _Node {
  /// libxml's xmlNodePtr
  node_ptr: *mut c_void,
  /// Reference to parent `Document`
  document: DocumentRef,
  unlinked: bool,
}

/// An xml node
#[derive(Clone, Debug)]
pub struct Node(NodeRef);

impl Hash for Node {
  /// Generates a hash value from the `node_ptr` value.
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.node_ptr().hash(state);
  }
}

impl PartialEq for Node {
  /// Two nodes are considered equal, if they point to the same xmlNode.
  fn eq(&self, other: &Node) -> bool {
    self.node_ptr() == other.node_ptr()
  }
}

impl Eq for Node {}

impl Drop for Node {
  /// Free node if it isn't bound in some document
  fn drop(&mut self) {
    if self.0.borrow().unlinked {
      let node_ptr = self.node_ptr_mut();
      if !node_ptr.is_null() {
        unsafe {
          xmlFreeNode(node_ptr);
        }
      }
    }
  }
}

pub(crate) type DocumentRef = Rc<RefCell<_Document>>;

// TODO: Do the fields need to be public in crate?
#[derive(Debug)]
pub(crate) struct _Document {
  /// libxml's `DocumentPtr`
  pub(crate) doc_ptr: *mut c_void,
  pub(crate) nodes: HashMap<*mut c_void, Node>,
}

impl _Document {
  /// Internal bookkeeping function, so far only used by `Node::wrap`
  pub(crate) fn insert_node(&mut self, node_ptr: *mut c_void, node: Node) {
    self.nodes.insert(node_ptr, node);
  }
}

/// A libxml2 Document
#[derive(Clone)]
pub struct Document(pub(crate) DocumentRef);

/*
#[derive(Debug)]
pub struct Document {
  /// libxml's `DocumentPtr`
  pub(crate) doc_ptr: *mut c_void,
}
*/

impl Drop for Document {
  ///Free document when it goes out of scope
  fn drop(&mut self) {
    unsafe {
      xmlFreeDoc(self.doc_ptr());
    }
  }
}

impl Document {
  /// Creates a new empty libxml2 document
  pub fn new() -> Result<Self, ()> {
    unsafe {
      let c_version = CString::new("1.0").unwrap();
      let doc_ptr = xmlNewDoc(c_version.as_ptr());
      if doc_ptr.is_null() {
        Err(())
      } else {
        let doc = _Document {
          doc_ptr,
          nodes: HashMap::new(),
        };
        Ok(Document(Rc::new(RefCell::new(doc))))
      }
    }
  }

  pub(crate) fn doc_ptr(&self) -> *mut c_void {
    self.0.borrow().doc_ptr
  }

  /// Creates a new `Document` from an existing libxml2 pointer
  pub fn new_ptr(doc_ptr: *mut c_void) -> Self {
    let doc = _Document {
      doc_ptr,
      nodes: HashMap::new(),
    };
    Document(Rc::new(RefCell::new(doc)))
  }
  /// Write document to `filename`
  pub fn save_file(&self, filename: &str) -> Result<c_int, ()> {
    let c_filename = CString::new(filename).unwrap();
    unsafe {
      let retval = xmlSaveFile(c_filename.as_ptr(), self.doc_ptr());
      if retval < 0 {
        return Err(());
      }
      Ok(retval)
    }
  }

  pub(crate) fn register_node(&self, node_ptr: *mut c_void) -> Node {
    Node::wrap(node_ptr, self.0.clone())
  }

  /// Get the root element of the document
  pub fn get_root_element(&self) -> Option<Node> {
    unsafe {
      let node_ptr = xmlDocGetRootElement(self.doc_ptr());
      if node_ptr.is_null() {
        None
      } else {
        Some(self.register_node(node_ptr))
      }
    }
  }

  /// Sets the root element of the document
  pub fn set_root_element(&mut self, root: &Node) {
    unsafe {
      xmlDocSetRootElement(self.doc_ptr(), root.node_ptr());
      // root.node_is_inserted = true;
    }
  }

  fn ptr_as_result(&mut self, node_ptr: *mut c_void) -> Result<Node, ()> {
    if node_ptr.is_null() {
      Err(())
    } else {
      let node = self.register_node(node_ptr);
      Ok(node)
    }
  }

  /// Import a `Node` from another `Document`
  pub fn import_node(&mut self, node: &mut Node) -> Result<Node, ()> {
    if !node.0.borrow_mut().unlinked {
      return Err(());
    }

    let node_ptr = unsafe { xmlDocCopyNode(node.node_ptr(), self.doc_ptr(), 1) };
    self.ptr_as_result(node_ptr)
  }

  /// Serializes the `Document`
  pub fn to_string(&self, format: bool) -> String {
    unsafe {
      // allocate a buffer to dump into
      let mut receiver = ptr::null_mut();
      let size: c_int = 0;
      let c_utf8 = CString::new("UTF-8").unwrap();

      if !format {
        xmlDocDumpMemoryEnc(self.doc_ptr(), &mut receiver, &size, c_utf8.as_ptr(), 1);
      } else {
        let current_indent = getIndentTreeOutput();
        setIndentTreeOutput(1);
        xmlDocDumpFormatMemoryEnc(self.doc_ptr(), &mut receiver, &size, c_utf8.as_ptr(), 1);
        setIndentTreeOutput(current_indent);
      }

      let c_string = CStr::from_ptr(receiver);
      let node_string = c_string.to_string_lossy().into_owned();
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
      xmlNodeDump(
        buf,
        self.doc_ptr(),
        node.node_ptr(),
        1, // level of indentation
        0, /* disable formatting */
      );
      let result_ptr = xmlBufferContent(buf);
      let c_string = CStr::from_ptr(result_ptr);
      let node_string = c_string.to_string_lossy().into_owned();
      xmlBufferFree(buf);

      node_string
    }
  }

  /// Creates a node for an XML processing instruction
  pub fn create_processing_instruction(&mut self, name: &str, content: &str) -> Result<Node, ()> {
    unsafe {
      let c_name = CString::new(name).unwrap();
      let c_content = CString::new(content).unwrap();

      let node_ptr = xmlNewDocPI(self.doc_ptr(), c_name.as_ptr(), c_content.as_ptr());
      if node_ptr.is_null() {
        Err(())
      } else {
        Ok(self.register_node(node_ptr))
      }
    }
  }

  // TODO: Discuss use case, this could probably cause problems
  /// Cast the document as a libxml Node
  pub fn as_node(&self) -> Node {
    // TODO: Memory management? Could be a major pain...
    self.register_node(self.doc_ptr())
  }
}

// TODO: Perhaps rename to copy or something. We can't impl the clone trait for anything else than cloning a Doucment reference =/
/*
impl Clone for Document {
  fn clone(&self) -> Self {
    let doc_ptr = unsafe { xmlCopyDoc(self.doc_ptr(), 1) };
    ptr_as_doc_opt(doc_ptr).expect("Could not clone the document!")
  }

  fn clone_from(&mut self, source: &Self) {
    if !self.doc_ptr().is_null() {
      panic!("Can only invoke clone_from on a Document struct with no pointer assigned.")
    }

    let doc_ptr = unsafe { xmlCopyDoc(source.doc_ptr, 1) };
    if doc_ptr.is_null() {
      panic!("Could not clone the Document!")
    }
    self.doc_ptr = doc_ptr;
  }
}
*/

/*
// The helper functions for trees
fn ptr_as_node_opt(node_ptr: *mut c_void) -> Option<Node> {
  if node_ptr.is_null() {
    None
  } else {
    Some(Node { node_ptr: node_ptr })
  }
}
*/

/*
// The helper functions for trees
fn ptr_as_doc_opt(doc_ptr: *mut c_void) -> Option<Document> {
  if doc_ptr.is_null() {
    None
  } else {
    Some(Document { doc_ptr })
  }
}
*/

/// Types of xml nodes
#[derive(Debug, PartialEq)]
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
      Some(ns) => ns.ns_ptr(),
    };
    unsafe {
      let node = xmlNewDocNode(doc.doc_ptr(), ns_ptr, c_name.as_ptr(), ptr::null());
      if node.is_null() {
        Err(())
      } else {
        Ok(Node::wrap(node, doc.0.clone()))
      }
    }
  }

  /// Return underlying libxml node ptr
  pub(crate) fn node_ptr(&self) -> *const c_void {
    self.0.borrow().node_ptr
  }

  /// Return underlying libxml node ptr
  pub(crate) fn node_ptr_mut(&mut self) -> *mut c_void {
    self.0.borrow_mut().node_ptr
  }

  /// Wrap a libxml node ptr with a Node
  pub(crate) fn wrap(node_ptr: *mut c_void, document: DocumentRef) -> Node {
    let node = _Node {
      node_ptr,
      document: document.clone(),
      unlinked: false,
    };
    let wrapped_node = Node(Rc::new(RefCell::new(node)));
    document
      .borrow_mut()
      .insert_node(node_ptr, wrapped_node.clone());
    wrapped_node
  }

  /// Create a new text node, bound to a given document
  pub fn new_text(content: &str, doc: &Document) -> Result<Self, ()> {
    // We will only allow to work with document-bound nodes for now, to avoid the problems of memory management.
    let c_content = CString::new(content).unwrap();
    unsafe {
      let node = xmlNewDocText(doc.doc_ptr(), c_content.as_ptr());
      if node.is_null() {
        Err(())
      } else {
        Ok(Node::wrap(node, doc.0.clone()))
      }
    }
  }
  /// Create a mock node, used for a placeholder argument
  pub fn mock(doc: &Document) -> Self {
    Node::new("mock", None, &doc).unwrap()
  }

  /// `libc::c_void` isn't hashable and cannot be made hashable
  pub fn to_hashable(&self) -> usize {
    unsafe { mem::transmute::<*const libc::c_void, usize>(self.node_ptr()) }
  }

  /// Returns the next sibling if it exists
  pub fn get_next_sibling(&self) -> Option<Node> {
    let ptr = unsafe { xmlNextSibling(self.node_ptr()) };
    self.ptr_as_option(ptr)
  }

  /// Returns the previous sibling if it exists
  pub fn get_prev_sibling(&self) -> Option<Node> {
    let ptr = unsafe { xmlPrevSibling(self.node_ptr()) };
    self.ptr_as_option(ptr)
  }

  /// Returns the first child if it exists
  pub fn get_first_child(&self) -> Option<Node> {
    let ptr = unsafe { xmlGetFirstChild(self.node_ptr()) };
    self.ptr_as_option(ptr)
  }

  /// Returns the first element child if it exists
  pub fn get_first_element_child(&self) -> Option<Node> {
    match self.get_first_child() {
      None => None,
      Some(child) => {
        let mut current_node = child;
        while !current_node.is_element_node() {
          if let Some(sibling) = current_node.get_next_sibling() {
            current_node = sibling;
          } else {
            break;
          }
        }
        if current_node.is_element_node() {
          Some(current_node)
        } else {
          None
        }
      }
    }
  }

  /// Returns the last child if it exists
  pub fn get_last_child(&self) -> Option<Node> {
    let ptr = unsafe { xmlGetLastChild(self.node_ptr()) };
    self.ptr_as_option(ptr)
  }

  /// Returns all child nodes of the given node as a vector
  pub fn get_child_nodes(&self) -> Vec<Node> {
    let mut children = Vec::new();
    if let Some(node) = self.get_first_child() {
      children.push(node.clone());
      let mut current_node = node;
      while let Some(sibling) = current_node.get_next_sibling() {
        current_node = sibling.clone();
        children.push(sibling)
      }
    }
    children
  }

  /// Returns all child elements of the given node as a vector
  pub fn get_child_elements(&self) -> Vec<Node> {
    self
      .get_child_nodes()
      .into_iter()
      .filter(|n| n.get_type() == Some(NodeType::ElementNode))
      .collect::<Vec<Node>>()
  }

  /// Returns the parent if it exists
  pub fn get_parent(&self) -> Option<Node> {
    let ptr = unsafe { xmlGetParent(self.node_ptr()) };
    self.ptr_as_option(ptr)
  }

  /// Get the node type
  pub fn get_type(&self) -> Option<NodeType> {
    NodeType::from_c_int(unsafe { xmlGetNodeType(self.node_ptr()) })
  }

  /// Add a previous sibling
  pub fn add_prev_sibling(&self, new_sibling: &mut Node) -> Result<(), ()> {
    new_sibling.set_linked();
    // TODO: Think of using a Result type, the libxml2 call returns NULL on error, or the child node on success
    unsafe {
      if xmlAddPrevSibling(self.node_ptr(), new_sibling.node_ptr()).is_null() {
        Err(())
      } else {
        Ok(())
      }
    }
  }

  /// Add a next sibling
  pub fn add_next_sibling(&self, new_sibling: &mut Node) -> Result<(), ()> {
    new_sibling.set_linked();
    // TODO: Think of using a Result type, the libxml2 call returns NULL on error, or the child node on success
    unsafe {
      if xmlAddNextSibling(self.node_ptr(), new_sibling.node_ptr()).is_null() {
        Err(())
      } else {
        Ok(())
      }
    }
  }

  /// Returns true iff it is a text node
  pub fn is_text_node(&self) -> bool {
    self.get_type() == Some(NodeType::TextNode)
  }

  /// Checks if the given node is an Element
  pub fn is_element_node(&self) -> bool {
    self.get_type() == Some(NodeType::ElementNode)
  }

  /// Returns the name of the node (empty string if name pointer is `NULL`)
  pub fn get_name(&self) -> String {
    let name_ptr = unsafe { xmlNodeGetName(self.node_ptr()) };
    if name_ptr.is_null() {
      return String::new();
    } //empty string
    let c_string = unsafe { CStr::from_ptr(name_ptr) };
    c_string.to_string_lossy().into_owned()
  }

  /// Sets the name of this `Node`
  pub fn set_name(&mut self, name: &str) {
    let c_name = CString::new(name).unwrap();
    unsafe { xmlNodeSetName(self.node_ptr_mut(), c_name.as_ptr()) }
  }

  /// Returns the content of the node
  /// (assumes UTF-8 XML document)
  pub fn get_content(&self) -> String {
    let content_ptr = unsafe { xmlNodeGetContent(self.node_ptr()) };
    if content_ptr.is_null() {
      //empty string when none
      return String::new();
    }
    let c_string = unsafe { CStr::from_ptr(content_ptr) };
    let rust_utf8 = c_string.to_string_lossy().into_owned();
    unsafe {
      libc::free(content_ptr as *mut c_void);
    }
    rust_utf8
  }

  /// Sets the text content of this `Node`
  pub fn set_content(&mut self, content: &str) {
    let c_content = CString::new(content).unwrap();
    unsafe { xmlNodeSetContent(self.node_ptr_mut(), c_content.as_ptr()) }
  }

  /// Returns the value of property `name`
  pub fn get_property(&self, name: &str) -> Option<String> {
    let c_name = CString::new(name).unwrap();
    let value_ptr = unsafe { xmlGetProp(self.node_ptr(), c_name.as_ptr()) };
    if value_ptr.is_null() {
      return None;
    }
    let c_value_string = unsafe { CStr::from_ptr(value_ptr) };
    let prop_str = c_value_string.to_string_lossy().into_owned();
    // TODO: Ensure all calls to `.to_string_lossy` are working on properly deallocated CStr instances.
    //       If that is not the case - memory will leak - can be checked with valgrind.
    //       A safe way to free the memory is using libc::free -- I have experienced that xmlFree from libxml2 is not reliable
    unsafe {
      libc::free(value_ptr as *mut c_void);
    }
    Some(prop_str)
  }

  /// Returns the value of property `name` in namespace `ns`
  pub fn get_property_ns(&self, name: &str, ns: &str) -> Option<String> {
    let c_name = CString::new(name).unwrap();
    let c_ns = CString::new(ns).unwrap();
    let value_ptr = unsafe { xmlGetNsProp(self.node_ptr(), c_name.as_ptr(), c_ns.as_ptr()) };
    if value_ptr.is_null() {
      return None;
    }
    let c_value_string = unsafe { CStr::from_ptr(value_ptr) };
    let prop_str = c_value_string.to_string_lossy().into_owned();
    unsafe {
      libc::free(value_ptr as *mut c_void);
    }
    Some(prop_str)
  }

  /// Return an attribute as a `Node` struct of type AttributeNode
  pub fn get_property_node(&self, name: &str) -> Option<Node> {
    let c_name = CString::new(name).unwrap();
    unsafe {
      let attr_node = xmlHasProp(self.node_ptr(), c_name.as_ptr());
      self.ptr_as_option(attr_node)
    }
  }

  /// Sets the value of property `name` to `value`
  pub fn set_property(&mut self, name: &str, value: &str) {
    let c_name = CString::new(name).unwrap();
    let c_value = CString::new(value).unwrap();
    unsafe { xmlSetProp(self.node_ptr(), c_name.as_ptr(), c_value.as_ptr()) };
  }
  /// Sets a namespaced attribute
  pub fn set_property_ns(&mut self, name: &str, value: &str, ns: &Namespace) {
    let c_name = CString::new(name).unwrap();
    let c_value = CString::new(value).unwrap();
    unsafe {
      xmlSetNsProp(
        self.node_ptr(),
        ns.ns_ptr(),
        c_name.as_ptr(),
        c_value.as_ptr(),
      )
    };
  }

  /// Removes the property of given `name`
  pub fn remove_property(&mut self, name: &str) {
    // TODO: Should we make the API return a Result type here?
    // Current behaviour on failures: silently return (noop)
    let c_name = CString::new(name).unwrap();
    unsafe {
      let attr_node = xmlHasProp(self.node_ptr(), c_name.as_ptr());
      if !attr_node.is_null() {
        xmlRemoveProp(attr_node);
      }
    }
  }

  /// Alias for get_property
  pub fn get_attribute(&self, name: &str) -> Option<String> {
    self.get_property(name)
  }
  /// Alias for get_property_ns
  pub fn get_attribute_ns(&self, name: &str, ns: &str) -> Option<String> {
    self.get_property_ns(name, ns)
  }

  /// Alias for get_property_node
  pub fn get_attribute_node(&self, name: &str) -> Option<Node> {
    self.get_property_node(name)
  }

  /// Alias for set_property
  pub fn set_attribute(&mut self, name: &str, value: &str) {
    self.set_property(name, value)
  }
  /// Alias for set_property_ns
  pub fn set_attribute_ns(&mut self, name: &str, value: &str, ns: &Namespace) {
    self.set_property_ns(name, value, ns)
  }

  /// Alias for remove_property
  pub fn remove_attribute(&mut self, name: &str) {
    self.remove_property(name)
  }

  /// Get a copy of the attributes of this node
  pub fn get_properties(&self) -> HashMap<String, String> {
    let mut attributes = HashMap::new();
    let mut attr_names = Vec::new();
    unsafe {
      let mut current_prop = xmlGetFirstProperty(self.node_ptr());
      while !current_prop.is_null() {
        let name_ptr = xmlAttrName(current_prop);
        let c_name_string = CStr::from_ptr(name_ptr);
        let name = c_name_string.to_string_lossy().into_owned();
        attr_names.push(name);
        current_prop = xmlNextPropertySibling(current_prop);
      }
    }

    for name in attr_names {
      let value = self.get_property(&name).unwrap_or_default();
      attributes.insert(name, value);
    }

    attributes
  }

  /// Alias for `get_properties`
  pub fn get_attributes(&self) -> HashMap<String, String> {
    self.get_properties()
  }

  /// Gets the active namespace associated of this node
  pub fn get_namespace(&self) -> Option<Namespace> {
    unsafe {
      let ns_ptr = xmlNodeNs(self.node_ptr());
      if ns_ptr.is_null() {
        None
      } else {
        Some(Namespace { ns_ptr })
      }
    }
  }

  /// Gets a list of namespaces associated with this node
  pub fn get_namespaces(&self, doc: &Document) -> Vec<Namespace> {
    let list_ptr_raw = unsafe { xmlGetNsList(doc.doc_ptr(), self.node_ptr()) };
    if list_ptr_raw.is_null() {
      Vec::new()
    } else {
      let mut namespaces = Vec::new();
      let mut ptr_iter = list_ptr_raw as *mut *mut c_void;
      unsafe {
        while !ptr_iter.is_null() && !(*ptr_iter).is_null() {
          namespaces.push(Namespace { ns_ptr: *ptr_iter });
          ptr_iter = ptr_iter.add(1);
        }
        /* TODO: valgrind suggests this technique isn't sufficiently fluent:
          ==114895== Conditional jump or move depends on uninitialised value(s)
          ==114895==    at 0x4E9962F: xmlFreeNs (in /usr/lib/x86_64-linux-gnu/libxml2.so.2.9.4)
          ==114895==    by 0x195CE8: libxml::tree::Node::get_namespaces (tree.rs:723)
          ==114895==    by 0x12E7B6: base_tests::can_work_with_namespaces (base_tests.rs:537)

          DG: I could not improve on this state without creating memory leaks after ~1 hour, so I am
          marking it as future work.
        */
        xmlFreeNs(list_ptr_raw);
      }
      namespaces
    }
  }

  /// Get a list of namespaces declared with this node
  pub fn get_namespace_declarations(&self) -> Vec<Namespace> {
    if self.get_type() != Some(NodeType::ElementNode) {
      // only element nodes can have declarations
      return Vec::new();
    }
    let mut namespaces = Vec::new();
    let mut ns_ptr = unsafe { xmlNodeNsDeclarations(self.node_ptr()) };
    while !ns_ptr.is_null() {
      unsafe {
        if !xmlNsPrefix(ns_ptr).is_null() || !xmlNsHref(ns_ptr).is_null() {
          namespaces.push(Namespace { ns_ptr });
        }
        ns_ptr = xmlNextNsSibling(ns_ptr);
      }
    }
    namespaces
  }

  /// Sets a `Namespace` for the node
  pub fn set_namespace(&mut self, namespace: &Namespace) {
    unsafe {
      xmlSetNs(self.node_ptr(), namespace.ns_ptr());
    }
  }

  /// Looks up the prefix of a namespace from its URI, basedo around a given `Node`
  pub fn lookup_namespace_prefix(&self, href: &str) -> Option<String> {
    if href.is_empty() {
      return None;
    }
    let c_href = CString::new(href).unwrap();
    unsafe {
      let ns_ptr = xmlSearchNsByHref(xmlGetDoc(self.node_ptr()), self.node_ptr(), c_href.as_ptr());
      if !ns_ptr.is_null() {
        let ns = Namespace { ns_ptr };
        let ns_prefix = ns.get_prefix();
        Some(ns_prefix)
      } else {
        None
      }
    }
  }

  /// Looks up the uri of a namespace from its prefix, basedo around a given `Node`
  pub fn lookup_namespace_uri(&self, prefix: &str) -> Option<String> {
    if prefix.is_empty() {
      return None;
    }
    let c_prefix = CString::new(prefix).unwrap();
    unsafe {
      let ns_ptr = xmlSearchNs(
        xmlGetDoc(self.node_ptr()),
        self.node_ptr(),
        c_prefix.as_ptr(),
      );
      if !ns_ptr.is_null() {
        let ns = Namespace { ns_ptr };
        let ns_prefix = ns.get_href();
        if !ns_prefix.is_empty() {
          Some(ns_prefix)
        } else {
          None
        }
      } else {
        None
      }
    }
  }

  // TODO: Clear a future Document namespaces vec
  /// Removes the namespaces of this `Node` and it's children!
  pub fn recursively_remove_namespaces(&mut self) {
    unsafe { xmlNodeRecursivelyRemoveNs(self.node_ptr_mut()) }
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
  pub fn add_child(&mut self, child: &mut Node) -> Result<Node, ()> {
    child.set_linked();
    unsafe {
      let new_child_ptr = xmlAddChild(self.node_ptr(), child.node_ptr());
      let new_child = self.ptr_as_option(new_child_ptr).ok_or(())?;
      Ok(new_child)
    }
  }

  /// Creates a new `Node` as child to the self `Node`
  pub fn new_child(&mut self, ns: Option<Namespace>, name: &str) -> Result<Node, ()> {
    let c_name = CString::new(name).unwrap();
    let ns_ptr = match ns {
      None => ptr::null_mut(),
      Some(ns) => ns.ns_ptr(),
    };
    unsafe {
      let new_ptr = xmlNewChild(self.node_ptr_mut(), ns_ptr, c_name.as_ptr(), ptr::null());
      Ok(Node::wrap(new_ptr, self.0.borrow().document.clone()))
    }
  }

  /// Adds a new text child, to this `Node`
  pub fn add_text_child(
    &mut self,
    ns: Option<Namespace>,
    name: &str,
    content: &str,
  ) -> Result<Node, ()> {
    let c_name = CString::new(name).unwrap();
    let c_content = CString::new(content).unwrap();
    let ns_ptr = match ns {
      None => ptr::null_mut(),
      Some(ns) => ns.ns_ptr(),
    };
    unsafe {
      let new_ptr = xmlNewTextChild(
        self.node_ptr_mut(),
        ns_ptr,
        c_name.as_ptr(),
        c_content.as_ptr(),
      );
      Ok(Node::wrap(new_ptr, self.0.borrow().document.clone()))
    }
  }

  /// Append text to this `Node`
  pub fn append_text(&mut self, content: &str) {
    let c_len = content.len() as i32;
    if c_len > 0 {
      let c_content = CString::new(content).unwrap();
      unsafe {
        xmlNodeAddContentLen(self.node_ptr(), c_content.as_ptr(), c_len);
      }
    }
  }

  /// Unbinds the Node from its siblings and Parent, but not from the Document it belongs to.
  ///   If the node is not inserted into the DOM afterwards, it will be lost after the program terminates.
  ///   From a low level view, the unbound node is stripped
  ///   from the context it is and inserted into a (hidden) document-fragment.
  pub fn unlink_node(&mut self) {
    let node_type = self.get_type();
    if node_type != Some(NodeType::DocumentNode) && node_type != Some(NodeType::DocumentFragNode) {
      if !self.0.borrow().unlinked {
        // only unlink nodes that are currently marked as linked
        self.set_unlinked();
        unsafe {
          xmlUnlinkNode(self.node_ptr());
        }
      }
    }
  }
  /// Alias for `unlink_node`
  pub fn unlink(&mut self) {
    self.unlink_node()
  }
  /// Alias for `unlink_node`
  pub fn unbind_node(&mut self) {
    self.unlink_node()
  }
  /// Alias for `unlink_node`
  pub fn unbind(&mut self) {
    self.unlink_node()
  }

  fn ptr_as_option(&self, node_ptr: *mut c_void) -> Option<Node> {
    if node_ptr.is_null() {
      None
    } else {
      let new_node = Node::wrap(node_ptr, self.0.borrow().document.clone());
      Some(new_node)
    }
  }

  /// internal helper to ensure the node is marked as linked/imported/adopted in the main document tree
  fn set_linked(&mut self) {
    self.0.borrow_mut().unlinked = false;
  }

  /// internal helper to ensure the node is marked as unlinked/removed from the main document tree
  fn set_unlinked(&mut self) {
    self.0.borrow_mut().unlinked = true;
  }
}

///An xml namespace
#[derive(Clone)]
pub struct Namespace {
  ///libxml's xmlNsPtr
  pub(crate) ns_ptr: *mut c_void,
}

impl Namespace {
  /// Creates a new namespace
  pub fn new(prefix: &str, href: &str, node: &mut Node) -> Result<Self, ()> {
    let c_href = CString::new(href).unwrap();
    let c_prefix = CString::new(prefix).unwrap();
    let c_prefix_ptr = if prefix.is_empty() {
      ptr::null()
    } else {
      c_prefix.as_ptr()
    };

    unsafe {
      let ns = xmlNewNs(node.node_ptr_mut(), c_href.as_ptr(), c_prefix_ptr);
      if ns.is_null() {
        Err(())
      } else {
        Ok(Namespace { ns_ptr: ns })
      }
    }
  }
  fn ns_ptr(&self) -> *mut c_void {
    self.ns_ptr
  }
  /// The namespace prefix
  pub fn get_prefix(&self) -> String {
    unsafe {
      let prefix_ptr = xmlNsPrefix(self.ns_ptr());
      if prefix_ptr.is_null() {
        String::new()
      } else {
        let c_prefix = CStr::from_ptr(prefix_ptr);
        c_prefix.to_string_lossy().into_owned()
      }
    }
  }

  /// The namespace href
  pub fn get_href(&self) -> String {
    unsafe {
      let href_ptr = xmlNsHref(self.ns_ptr());
      if href_ptr.is_null() {
        String::new()
      } else {
        let c_href = CStr::from_ptr(href_ptr);
        c_href.to_string_lossy().into_owned()
      }
    }
  }

  /// Explicit free method, until (if?) we implement automatic+safe free-on-drop
  pub fn free(&mut self) {
    unsafe { xmlFreeNs(self.ns_ptr()) }
  }
}
