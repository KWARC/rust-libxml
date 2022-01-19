//! Node, and related, feature set
//!
use libc::{c_char, c_void};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::ffi::{CStr, CString};
use std::hash::{Hash, Hasher};
use std::ptr;
use std::rc::Rc;
use std::str;

use crate::bindings::*;
use crate::c_helpers::*;
use crate::tree::namespace::Namespace;
use crate::tree::nodetype::NodeType;
use crate::tree::{Document, DocumentRef, DocumentWeak};
use crate::xpath::Context;

/// Guard treshold for enforcing runtime mutability checks for Nodes
pub static mut NODE_RC_MAX_GUARD: usize = 2;

/// Set the guard value for the max Rc "strong count" allowed for mutable use of a Node
/// Default is 2
pub fn set_node_rc_guard(value: usize) {
  unsafe {
    NODE_RC_MAX_GUARD = value;
  }
}

type NodeRef = Rc<RefCell<_Node>>;

#[derive(Debug)]
struct _Node {
  /// libxml's xmlNodePtr
  node_ptr: xmlNodePtr,
  /// Reference to parent `Document`
  document: DocumentWeak,
  /// Bookkeep removal from a parent
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

impl Drop for _Node {
  /// Free node if it isn't bound in some document
  /// Warning: xmlFreeNode is RECURSIVE into the node's children, so this may lead to segfaults if used carelessly
  fn drop(&mut self) {
    if self.unlinked {
      let node_ptr = self.node_ptr;
      if !node_ptr.is_null() {
        unsafe {
          xmlFreeNode(node_ptr);
        }
      }
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
      let node = xmlNewDocRawNode(
        doc.doc_ptr(),
        ns_ptr,
        c_name.as_bytes().as_ptr(),
        ptr::null(),
      );
      if node.is_null() {
        Err(())
      } else {
        Ok(Node::wrap_new(node, &doc.0))
      }
    }
  }

  /// Immutably borrows the underlying libxml2 `xmlNodePtr` pointer
  pub fn node_ptr(&self) -> xmlNodePtr {
    self.0.borrow().node_ptr
  }

  /// Mutably borrows the underlying libxml2 `xmlNodePtr` pointer
  /// Also protects against mutability conflicts at runtime.
  pub fn node_ptr_mut(&mut self) -> Result<xmlNodePtr, String> {
    let weak_count = Rc::weak_count(&self.0);
    let strong_count = Rc::strong_count(&self.0);

    // The basic idea would be to use `Rc::get_mut` to guard against multiple borrows.
    // However, our approach to bookkeeping nodes implies there is *always* a second Rc reference
    // in the document.nodes Hash. So rather than use `get_mut` directly, the
    // correct check would be to have a weak count of 0 and a strong count <=2 (one for self, one for .nodes)
    let guard_ok = unsafe { weak_count == 0 && strong_count <= NODE_RC_MAX_GUARD };
    if guard_ok {
      Ok(self.0.borrow_mut().node_ptr)
    } else {
      Err(format!(
        "Can not mutably reference a shared Node {:?}! Rc: weak count: {:?}; strong count: {:?}",
        self.get_name(),
        weak_count,
        strong_count,
      ))
    }
  }

  /// Wrap a libxml node ptr with a Node
  fn _wrap(node_ptr: xmlNodePtr, unlinked: bool, document: &DocumentRef) -> Node {
    // If already seen, return saved Node
    if let Some(node) = document.borrow().get_node(node_ptr) {
      return node.clone();
    }
    // If newly encountered pointer, wrap
    let node = _Node {
      node_ptr,
      document: Rc::downgrade(&document),
      unlinked,
    };
    let wrapped_node = Node(Rc::new(RefCell::new(node)));
    document
      .borrow_mut()
      .insert_node(node_ptr, wrapped_node.clone());
    wrapped_node
  }
  /// Wrap a node already linked to a `document` tree
  pub(crate) fn wrap(node_ptr: xmlNodePtr, document: &DocumentRef) -> Node {
    Node::_wrap(node_ptr, false, document)
  }
  /// Wrap, a node owned by, but not yet linked to, a `document`
  pub(crate) fn wrap_new(node_ptr: xmlNodePtr, document: &DocumentRef) -> Node {
    Node::_wrap(node_ptr, true, document)
  }

  /// Create a new text node, bound to a given document
  pub fn new_text(content: &str, doc: &Document) -> Result<Self, ()> {
    // We will only allow to work with document-bound nodes for now, to avoid the problems of memory management.
    let c_content = CString::new(content).unwrap();
    unsafe {
      let node = xmlNewDocText(doc.doc_ptr(), c_content.as_bytes().as_ptr());
      if node.is_null() {
        Err(())
      } else {
        Ok(Node::wrap_new(node, &doc.0))
      }
    }
  }
  /// Create a mock node, used for a placeholder argument
  pub fn mock(doc: &Document) -> Self {
    Node::new("mock", None, &doc).unwrap()
  }

  /// Create a mock node, used for a placeholder argument
  pub fn null() -> Self {
    Node(Rc::new(RefCell::new(_Node {
      node_ptr: ptr::null_mut(),
      document: Rc::downgrade(&Document::null_ref()),
      unlinked: true,
    })))
  }

  /// `libc::c_void` isn't hashable and cannot be made hashable
  pub fn to_hashable(&self) -> usize {
    self.node_ptr() as usize
  }

  pub(crate) fn get_docref(&self) -> DocumentWeak {
    self.0.borrow().document.clone()
  }

  /// Returns the next sibling if it exists
  pub fn get_next_sibling(&self) -> Option<Node> {
    let ptr = xmlNextSibling(self.node_ptr());
    self.ptr_as_option(ptr)
  }

  /// Returns the previous sibling if it exists
  pub fn get_prev_sibling(&self) -> Option<Node> {
    let ptr = xmlPrevSibling(self.node_ptr());
    self.ptr_as_option(ptr)
  }

  /// Returns the first child if it exists
  pub fn get_first_child(&self) -> Option<Node> {
    let ptr = xmlGetFirstChild(self.node_ptr());
    self.ptr_as_option(ptr)
  }
  /// Returns the last child if it exists
  pub fn get_last_child(&self) -> Option<Node> {
    let ptr = unsafe { xmlGetLastChild(self.node_ptr()) };
    self.ptr_as_option(ptr)
  }

  /// Returns the next element sibling if it exists
  pub fn get_next_element_sibling(&self) -> Option<Node> {
    match self.get_next_sibling() {
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

  /// Returns the previous element sibling if it exists
  pub fn get_prev_element_sibling(&self) -> Option<Node> {
    match self.get_prev_sibling() {
      None => None,
      Some(child) => {
        let mut current_node = child;
        while !current_node.is_element_node() {
          if let Some(sibling) = current_node.get_prev_sibling() {
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

  /// Returns the last element child if it exists
  pub fn get_last_element_child(&self) -> Option<Node> {
    match self.get_last_child() {
      None => None,
      Some(child) => {
        let mut current_node = child;
        while !current_node.is_element_node() {
          if let Some(sibling) = current_node.get_prev_sibling() {
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

  /// Returns all child nodes of the given node as a vector
  pub fn get_child_nodes(&self) -> Vec<Node> {
    let mut children = Vec::new();
    if let Some(first_child) = self.get_first_child() {
      children.push(first_child);
      while let Some(sibling) = children.last().unwrap().get_next_sibling() {
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
    let ptr = xmlGetParent(self.node_ptr());
    self.ptr_as_option(ptr)
  }

  /// Get the node type
  pub fn get_type(&self) -> Option<NodeType> {
    NodeType::from_int(xmlGetNodeType(self.node_ptr()))
  }

  /// Add a previous sibling
  pub fn add_prev_sibling(&mut self, new_sibling: &mut Node) -> Result<(), Box<dyn Error + Send + Sync>> {
    new_sibling.set_linked();
    unsafe {
      if xmlAddPrevSibling(self.node_ptr_mut()?, new_sibling.node_ptr_mut()?).is_null() {
        Err(From::from("add_prev_sibling returned NULL"))
      } else {
        Ok(())
      }
    }
  }

  /// Add a next sibling
  pub fn add_next_sibling(&mut self, new_sibling: &mut Node) -> Result<(), Box<dyn Error + Send + Sync>> {
    new_sibling.set_linked();
    unsafe {
      if xmlAddNextSibling(self.node_ptr_mut()?, new_sibling.node_ptr_mut()?).is_null() {
        Err(From::from("add_next_sibling returned NULL"))
      } else {
        Ok(())
      }
    }
  }

  /// Returns true if it is a text node
  pub fn is_text_node(&self) -> bool {
    self.get_type() == Some(NodeType::TextNode)
  }

  /// Checks if the given node is an Element
  pub fn is_element_node(&self) -> bool {
    self.get_type() == Some(NodeType::ElementNode)
  }

  /// Checks if the underlying libxml2 pointer is `NULL`
  pub fn is_null(&self) -> bool {
    self.node_ptr().is_null()
  }

  /// Returns the name of the node (empty string if name pointer is `NULL`)
  pub fn get_name(&self) -> String {
    let name_ptr = xmlNodeGetName(self.node_ptr());
    if name_ptr.is_null() {
      return String::new();
    } //empty string
    let c_string = unsafe { CStr::from_ptr(name_ptr) };
    c_string.to_string_lossy().into_owned()
  }

  /// Sets the name of this `Node`
  pub fn set_name(&mut self, name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name).unwrap();
    unsafe { xmlNodeSetName(self.node_ptr_mut()?, c_name.as_bytes().as_ptr()) }
    Ok(())
  }

  /// Returns the content of the node
  /// (assumes UTF-8 XML document)
  pub fn get_content(&self) -> String {
    let content_ptr = unsafe { xmlNodeGetContent(self.node_ptr()) };
    if content_ptr.is_null() {
      //empty string when none
      return String::new();
    }
    let c_string = unsafe { CStr::from_ptr(content_ptr as *const c_char) };
    let rust_utf8 = c_string.to_string_lossy().into_owned();
    unsafe {
      libc::free(content_ptr as *mut c_void);
    }
    rust_utf8
  }

  /// Sets the text content of this `Node`
  pub fn set_content(&mut self, content: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_content = CString::new(content).unwrap();
    unsafe { xmlNodeSetContent(self.node_ptr_mut()?, c_content.as_bytes().as_ptr()) }
    Ok(())
  }

  /// Returns the value of property `name`
  pub fn get_property(&self, name: &str) -> Option<String> {
    let c_name = CString::new(name).unwrap();
    let value_ptr = unsafe { xmlGetProp(self.node_ptr(), c_name.as_bytes().as_ptr()) };
    if value_ptr.is_null() {
      return None;
    }
    let c_value_string = unsafe { CStr::from_ptr(value_ptr as *const c_char) };
    let prop_str = c_value_string.to_string_lossy().into_owned();
    // A safe way to free the memory is using libc::free -- I have experienced that xmlFree from libxml2 is not reliable
    unsafe {
      libc::free(value_ptr as *mut c_void);
    }
    Some(prop_str)
  }

  /// Returns the value of property `name` in namespace `ns`
  pub fn get_property_ns(&self, name: &str, ns: &str) -> Option<String> {
    let c_name = CString::new(name).unwrap();
    let c_ns = CString::new(ns).unwrap();
    let value_ptr = unsafe {
      xmlGetNsProp(
        self.node_ptr(),
        c_name.as_bytes().as_ptr(),
        c_ns.as_bytes().as_ptr(),
      )
    };
    if value_ptr.is_null() {
      return None;
    }
    let c_value_string = unsafe { CStr::from_ptr(value_ptr as *const c_char) };
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
      let attr_node = xmlHasProp(self.node_ptr(), c_name.as_bytes().as_ptr());
      self.ptr_as_option(attr_node as xmlNodePtr)
    }
  }

  /// Sets the value of property `name` to `value`
  pub fn set_property(&mut self, name: &str, value: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name).unwrap();
    let c_value = CString::new(value).unwrap();
    unsafe {
      xmlSetProp(
        self.node_ptr_mut()?,
        c_name.as_bytes().as_ptr(),
        c_value.as_bytes().as_ptr(),
      )
    };
    Ok(())
  }
  /// Sets a namespaced attribute
  pub fn set_property_ns(
    &mut self,
    name: &str,
    value: &str,
    ns: &Namespace,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name).unwrap();
    let c_value = CString::new(value).unwrap();
    unsafe {
      xmlSetNsProp(
        self.node_ptr_mut()?,
        ns.ns_ptr(),
        c_name.as_bytes().as_ptr(),
        c_value.as_bytes().as_ptr(),
      )
    };
    Ok(())
  }

  /// Removes the property of given `name`
  pub fn remove_property(&mut self, name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name).unwrap();
    unsafe {
      let attr_node = xmlHasProp(self.node_ptr_mut()?, c_name.as_bytes().as_ptr());
      if !attr_node.is_null() {
        let remove_prop_status = xmlRemoveProp(attr_node);
        if remove_prop_status == 0 {
          Ok(())
        } else {
          // Propagate libxml2 failure to remove
          Err(From::from(format!(
            "libxml2 failed to remove property with status: {:?}",
            remove_prop_status
          )))
        }
      } else {
        // silently no-op if asked to remove a property which is not present
        Ok(())
      }
    }
  }

  /// Removes the property of given `name` and namespace (`ns`)
  pub fn remove_property_ns(&mut self, name: &str, ns: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name).unwrap();
    let c_ns = CString::new(ns).unwrap();
    unsafe {
      let attr_node = xmlHasNsProp(
          self.node_ptr_mut()?,
          c_name.as_bytes().as_ptr(),
          c_ns.as_bytes().as_ptr(),
      );
      if !attr_node.is_null() {
        let remove_prop_status = xmlRemoveProp(attr_node);
        if remove_prop_status == 0 {
          Ok(())
        } else {
          // Propagate libxml2 failure to remove
          Err(From::from(format!(
            "libxml2 failed to remove property with status: {:?}",
            remove_prop_status
          )))
        }
      } else {
        // silently no-op if asked to remove a property which is not present
        Ok(())
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
  pub fn set_attribute(&mut self, name: &str, value: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    self.set_property(name, value)
  }
  /// Alias for set_property_ns
  pub fn set_attribute_ns(
    &mut self,
    name: &str,
    value: &str,
    ns: &Namespace,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    self.set_property_ns(name, value, ns)
  }

  /// Alias for remove_property
  pub fn remove_attribute(&mut self, name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    self.remove_property(name)
  }

  /// Alias for remove_property_ns
  pub fn remove_attribute_ns(&mut self, name: &str, ns: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    self.remove_property_ns(name, ns)
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
    let ns_ptr = xmlNodeNs(self.node_ptr());
    if ns_ptr.is_null() {
      None
    } else {
      Some(Namespace { ns_ptr })
    }
  }

  /// Gets a list of namespaces associated with this node
  pub fn get_namespaces(&self, doc: &Document) -> Vec<Namespace> {
    let list_ptr_raw = unsafe { xmlGetNsList(doc.doc_ptr(), self.node_ptr()) };
    if list_ptr_raw.is_null() {
      Vec::new()
    } else {
      let mut namespaces = Vec::new();
      let mut ptr_iter = list_ptr_raw as *mut xmlNsPtr;
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
        /* TODO: How do we properly deallocate here? The approach bellow reliably segfaults tree_tests on 1 thread */
        // println!("\n-- xmlfreens on : {:?}", list_ptr_raw);
        // xmlFreeNs(list_ptr_raw as xmlNsPtr);
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
    let mut ns_ptr = xmlNodeNsDeclarations(self.node_ptr());
    while !ns_ptr.is_null() {
      if !xmlNsPrefix(ns_ptr).is_null() || !xmlNsHref(ns_ptr).is_null() {
        namespaces.push(Namespace { ns_ptr });
      }
      ns_ptr = xmlNextNsSibling(ns_ptr);
    }
    namespaces
  }

  /// Sets a `Namespace` for the node
  pub fn set_namespace(&mut self, namespace: &Namespace) -> Result<(), Box<dyn Error + Send + Sync>> {
    unsafe {
      xmlSetNs(self.node_ptr_mut()?, namespace.ns_ptr());
    }
    Ok(())
  }

  /// Looks up the prefix of a namespace from its URI, basedo around a given `Node`
  pub fn lookup_namespace_prefix(&self, href: &str) -> Option<String> {
    if href.is_empty() {
      return None;
    }
    let c_href = CString::new(href).unwrap();
    unsafe {
      let ptr_mut = self.node_ptr();
      let ns_ptr = xmlSearchNsByHref(xmlGetDoc(ptr_mut), ptr_mut, c_href.as_bytes().as_ptr());
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
        c_prefix.as_bytes().as_ptr(),
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
  pub fn recursively_remove_namespaces(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
    xmlNodeRecursivelyRemoveNs(self.node_ptr_mut()?);
    Ok(())
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
  pub fn add_child(&mut self, child: &mut Node) -> Result<(), String> {
    child.set_linked();
    unsafe {
      let new_child_ptr = xmlAddChild(self.node_ptr_mut()?, child.node_ptr_mut()?);
      if new_child_ptr.is_null() {
        Err("add_child encountered NULL pointer".to_string())
      } else {
        Ok(())
      }
    }
  }

  /// Creates a new `Node` as child to the self `Node`
  pub fn new_child(&mut self, ns: Option<Namespace>, name: &str) -> Result<Node, Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name).unwrap();
    let ns_ptr = match ns {
      None => ptr::null_mut(),
      Some(mut ns) => ns.ns_ptr_mut(),
    };
    unsafe {
      let new_ptr = xmlNewChild(
        self.node_ptr_mut()?,
        ns_ptr,
        c_name.as_bytes().as_ptr(),
        ptr::null(),
      );
      Ok(Node::wrap(new_ptr, &self.get_docref().upgrade().unwrap()))
    }
  }

  /// Adds a new text child, to this `Node`
  pub fn add_text_child(
    &mut self,
    ns: Option<Namespace>,
    name: &str,
    content: &str,
  ) -> Result<Node, Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name).unwrap();
    let c_content = CString::new(content).unwrap();
    let ns_ptr = match ns {
      None => ptr::null_mut(),
      Some(mut ns) => ns.ns_ptr_mut(),
    };
    unsafe {
      let new_ptr = xmlNewTextChild(
        self.node_ptr_mut()?,
        ns_ptr,
        c_name.as_bytes().as_ptr(),
        c_content.as_bytes().as_ptr(),
      );
      Ok(Node::wrap(new_ptr, &self.get_docref().upgrade().unwrap()))
    }
  }

  /// Append text to this `Node`
  pub fn append_text(&mut self, content: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_len = content.len() as i32;
    if c_len > 0 {
      let c_content = CString::new(content).unwrap();
      unsafe {
        xmlNodeAddContentLen(self.node_ptr_mut()?, c_content.as_bytes().as_ptr(), c_len);
      }
    }
    Ok(())
  }

  /// Unbinds the Node from its siblings and Parent, but not from the Document it belongs to.
  ///   If the node is not inserted into the DOM afterwards, it will be lost after the program terminates.
  ///   From a low level view, the unbound node is stripped
  ///   from the context it is and inserted into a (hidden) document-fragment.
  pub fn unlink_node(&mut self) {
    let node_type = self.get_type();
    if node_type != Some(NodeType::DocumentNode)
      && node_type != Some(NodeType::DocumentFragNode)
      && !self.is_unlinked()
    {
      // only unlink nodes that are currently marked as linked
      self.set_unlinked();
      unsafe {
        xmlUnlinkNode(self.node_ptr());
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

  /// Checks if node is marked as unlinked
  pub fn is_unlinked(&self) -> bool {
    self.0.borrow().unlinked
  }

  fn ptr_as_option(&self, node_ptr: xmlNodePtr) -> Option<Node> {
    if node_ptr.is_null() {
      None
    } else {
      let doc_ref = self.get_docref().upgrade().unwrap();
      let new_node = Node::wrap(node_ptr, &doc_ref);
      Some(new_node)
    }
  }

  /// internal helper to ensure the node is marked as linked/imported/adopted in the main document tree
  pub(crate) fn set_linked(&self) {
    self.0.borrow_mut().unlinked = false;
  }

  /// internal helper to ensure the node is marked as unlinked/removed from the main document tree
  pub(crate) fn set_unlinked(&self) {
    self.0.borrow_mut().unlinked = true;
    self
      .get_docref()
      .upgrade()
      .unwrap()
      .borrow_mut()
      .forget_node(self.node_ptr());
  }

  /// find nodes via xpath, at a specified node or the document root
  pub fn findnodes(&self, xpath: &str) -> Result<Vec<Node>, ()> {
    let mut context = Context::from_node(&self)?;
    context.findnodes(xpath, Some(self))
  }

  /// replace a `self`'s `old` child node with a `new` node in the same position
  /// borrowed from Perl's XML::LibXML
  pub fn replace_child_node(
    &mut self,
    mut new: Node,
    mut old: Node,
  ) -> Result<Node, Box<dyn Error + Send + Sync>> {
    // if newNode == oldNode or self == newNode then do nothing, just return nNode.
    if new == old || self == &new {
      // nothing to do here, already in place
      Ok(old)
    } else if self.get_type() == Some(NodeType::ElementNode) {
      if let Some(old_parent) = old.get_parent() {
        if &old_parent == self {
          // unlink new to be available for insertion
          new.unlink();
          // mid-child case
          old.add_next_sibling(&mut new)?;
          old.unlink();
          Ok(old)
        } else {
          Err(From::from(format!(
            "Old node was not a child of {:?} parent. Registered parent is {:?} instead.",
            self.get_name(),
            old_parent.get_name()
          )))
        }
      } else {
        Err(From::from(format!(
          "Old node was not a child of {:?} parent. No registered parent exists.",
          self.get_name()
        )))
      }
    } else {
      Err(From::from(
        "Can only call replace_child_node an a NodeType::Element type parent.",
      ))
    }
  }
}
