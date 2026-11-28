use libc::{c_char, c_void};
use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::ptr;
use std::str;

use crate::bindings::*;
use crate::c_helpers::*;
use crate::tree::Document;
use crate::tree::namespace::{self, Namespace};
use crate::tree::node::{
  attr_node_value, single_text_value, with_c_str_or_else, with_c_strs_or_else, with_property_copy,
  with_str_copy,
};
use crate::tree::nodetype::NodeType;
use crate::xpath::Context;

/// Lightweight struct for read-only parallel processing
///
/// Navigating a `RoNode` does not touch the document's `Node` cache. Convert with
/// [`Node::as_readonly`](crate::tree::Node::as_readonly) /
/// [`Document::node_from_readonly`](crate::tree::Document::node_from_readonly); read without allocating, in the common
/// case, through the `with_*` and `for_each_*` methods. A `RoNode` is valid while its document is alive and its node is
/// not freed. The thread that owns the document must not modify it while another thread reads it through a `RoNode`.
#[derive(Debug, Copy, Clone)]
pub struct RoNode(pub(crate) xmlNodePtr);

// we claim Sync and Send, as we are in read-only mode over the owning document
unsafe impl Sync for RoNode {}
unsafe impl Send for RoNode {}

impl PartialEq for RoNode {
  /// Two nodes are considered equal, if they point to the same xmlNode.
  fn eq(&self, other: &RoNode) -> bool {
    std::ptr::eq(self.0, other.0)
  }
}
impl Eq for RoNode {}

impl RoNode {
  /// Immutably borrows the underlying libxml2 `xmlNodePtr` pointer
  pub fn node_ptr(&self) -> xmlNodePtr {
    self.0
  }

  /// Returns the next sibling if it exists
  pub fn get_next_sibling(self) -> Option<RoNode> {
    let ptr = xmlNextSibling(self.0);
    self.ptr_as_option(ptr)
  }

  /// Returns the previous sibling if it exists
  pub fn get_prev_sibling(self) -> Option<RoNode> {
    let ptr = xmlPrevSibling(self.0);
    self.ptr_as_option(ptr)
  }

  /// Returns the first child if it exists
  pub fn get_first_child(self) -> Option<RoNode> {
    let ptr = xmlGetFirstChild(self.0);
    self.ptr_as_option(ptr)
  }

  /// Returns the last child if it exists
  pub fn get_last_child(self) -> Option<RoNode> {
    let ptr = unsafe { xmlGetLastChild(self.0) };
    self.ptr_as_option(ptr)
  }

  /// Returns the next element sibling if it exists
  pub fn get_next_element_sibling(&self) -> Option<RoNode> {
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
  pub fn get_prev_element_sibling(&self) -> Option<RoNode> {
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
  pub fn get_first_element_child(self) -> Option<RoNode> {
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
  pub fn get_last_element_child(&self) -> Option<RoNode> {
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
  pub fn get_child_nodes(self) -> Vec<RoNode> {
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
  pub fn get_child_elements(self) -> Vec<RoNode> {
    self
      .get_child_nodes()
      .into_iter()
      .filter(|n| n.get_type() == Some(NodeType::ElementNode))
      .collect::<Vec<RoNode>>()
  }

  /// Returns the parent if it exists
  pub fn get_parent(self) -> Option<RoNode> {
    let ptr = xmlGetParent(self.0);
    self.ptr_as_option(ptr)
  }

  /// Get the node type
  pub fn get_type(self) -> Option<NodeType> {
    NodeType::from_int(xmlGetNodeType(self.0))
  }

  /// Returns true if it is a text node
  pub fn is_text_node(self) -> bool {
    self.get_type() == Some(NodeType::TextNode)
  }

  /// Checks if the given node is an Element
  pub fn is_element_node(self) -> bool {
    self.get_type() == Some(NodeType::ElementNode)
  }

  /// Checks if the underlying libxml2 pointer is `NULL`
  pub fn is_null(self) -> bool {
    self.0.is_null()
  }

  /// Returns the name of the node (empty string if name pointer is `NULL`)
  pub fn get_name(self) -> String {
    let name_ptr = xmlNodeGetName(self.0);
    if name_ptr.is_null() {
      return String::new();
    } //empty string
    let c_string = unsafe { CStr::from_ptr(name_ptr) };
    c_string.to_string_lossy().into_owned()
  }

  /// Returns the content of the node
  /// (assumes UTF-8 XML document)
  pub fn get_content(self) -> String {
    let content_ptr = unsafe { xmlNodeGetContent(self.0) };
    if content_ptr.is_null() {
      //empty string when none
      return String::new();
    }
    let c_string = unsafe { CStr::from_ptr(content_ptr as *const c_char) };
    let rust_utf8 = c_string.to_string_lossy().into_owned();
    bindgenFree(content_ptr as *mut c_void);
    rust_utf8
  }

  /// Returns the value of property `name`
  pub fn get_property(self, name: &str) -> Option<String> {
    let value_ptr = with_c_str_or_else(
      name,
      |_| ptr::null_mut(),
      |c_name| unsafe { xmlGetProp(self.0, c_name) },
    );
    if value_ptr.is_null() {
      return None;
    }
    let c_value_string = unsafe { CStr::from_ptr(value_ptr as *const c_char) };
    let prop_str = c_value_string.to_string_lossy().into_owned();
    bindgenFree(value_ptr as *mut c_void);
    Some(prop_str)
  }

  /// Returns the value of property `name` in namespace `ns`
  pub fn get_property_ns(self, name: &str, ns: &str) -> Option<String> {
    let value_ptr = with_c_strs_or_else(
      name,
      ns,
      |_| ptr::null_mut(),
      |c_name, c_ns| unsafe { xmlGetNsProp(self.0, c_name, c_ns) },
    );
    if value_ptr.is_null() {
      return None;
    }
    let c_value_string = unsafe { CStr::from_ptr(value_ptr as *const c_char) };
    let prop_str = c_value_string.to_string_lossy().into_owned();
    bindgenFree(value_ptr as *mut c_void);
    Some(prop_str)
  }

  /// Returns the value of property `name` with no namespace
  pub fn get_property_no_ns(self, name: &str) -> Option<String> {
    let value_ptr = with_c_str_or_else(
      name,
      |_| ptr::null_mut(),
      |c_name| unsafe { xmlGetNoNsProp(self.0, c_name) },
    );
    if value_ptr.is_null() {
      return None;
    }
    let c_value_string = unsafe { CStr::from_ptr(value_ptr as *const c_char) };
    let prop_str = c_value_string.to_string_lossy().into_owned();
    bindgenFree(value_ptr as *mut c_void);
    Some(prop_str)
  }

  /// Return an attribute as a `Node` struct of type AttributeNode
  ///
  /// A property that only the DTD gives a default value is returned as libxml2 finds it (`xmlHasProp`,
  /// `xmlHasNsProp`): as its declaration in the DTD, of type `AttributeDecl`.
  pub fn get_property_node(self, name: &str) -> Option<RoNode> {
    let attr_node = with_c_str_or_else(
      name,
      |_| ptr::null_mut(),
      |c_name| unsafe { xmlHasProp(self.0, c_name) },
    );
    self.ptr_as_option(attr_node as xmlNodePtr)
  }

  /// Return an attribute in a namespace `ns` as a `Node` of type AttributeNode
  ///
  /// A property that only the DTD gives a default value is returned as libxml2 finds it (`xmlHasProp`,
  /// `xmlHasNsProp`): as its declaration in the DTD, of type `AttributeDecl`.
  pub fn get_property_node_ns(self, name: &str, ns: &str) -> Option<RoNode> {
    let attr_node = with_c_strs_or_else(
      name,
      ns,
      |_| ptr::null_mut(),
      |c_name, c_ns| unsafe { xmlHasNsProp(self.0, c_name, c_ns) },
    );
    self.ptr_as_option(attr_node as xmlNodePtr)
  }

  /// Return an attribute with no namespace as a `Node` of type AttributeNode
  ///
  /// A property that only the DTD gives a default value is returned as libxml2 finds it (`xmlHasProp`,
  /// `xmlHasNsProp`): as its declaration in the DTD, of type `AttributeDecl`.
  pub fn get_property_node_no_ns(self, name: &str) -> Option<RoNode> {
    let attr_node = with_c_str_or_else(
      name,
      |_| ptr::null_mut(),
      |c_name| unsafe { xmlHasNsProp(self.0, c_name, ptr::null()) },
    );
    self.ptr_as_option(attr_node as xmlNodePtr)
  }

  /// Read property `name`, as [`RoNode::get_property`] does, and pass its value to `f`, copied on the stack when it is
  /// at most 128 bytes, so the common case allocates nothing; `None` when the node has no such property. As
  /// [`Node::with_property`](crate::tree::Node::with_property).
  #[must_use]
  pub fn with_property<R>(self, name: &CStr, f: impl FnOnce(&str) -> R) -> Option<R> {
    with_property_copy(self.0, name, f)
  }

  /// Alias for with_property
  #[must_use]
  pub fn with_attribute<R>(self, name: &CStr, f: impl FnOnce(&str) -> R) -> Option<R> {
    self.with_property(name, f)
  }

  /// Alias for get_property
  pub fn get_attribute(self, name: &str) -> Option<String> {
    self.get_property(name)
  }

  /// Alias for get_property_ns
  pub fn get_attribute_ns(self, name: &str, ns: &str) -> Option<String> {
    self.get_property_ns(name, ns)
  }

  /// Alias for get_property_no_ns
  pub fn get_attribute_no_ns(self, name: &str) -> Option<String> {
    self.get_property_no_ns(name)
  }

  /// Alias for get_property_node
  pub fn get_attribute_node(self, name: &str) -> Option<RoNode> {
    self.get_property_node(name)
  }

  /// Alias for get_property_node_ns
  pub fn get_attribute_node_ns(self, name: &str, ns: &str) -> Option<RoNode> {
    self.get_property_node_ns(name, ns)
  }

  /// Alias for get_property_node_no_ns
  pub fn get_attribute_node_no_ns(self, name: &str) -> Option<RoNode> {
    self.get_property_node_no_ns(name)
  }

  /// Get a copy of the attributes of this node
  pub fn get_properties(self) -> HashMap<String, String> {
    let mut attributes = HashMap::new();

    let mut current_prop = xmlGetFirstProperty(self.0);
    while !current_prop.is_null() {
      let name_ptr = xmlAttrName(current_prop);
      if name_ptr.is_null() {
        // A property with a NULL name can't be keyed; skip it rather than
        // `strlen(NULL)` inside `CStr::from_ptr`.
        current_prop = xmlNextPropertySibling(current_prop);
        continue;
      }
      let c_name_string = unsafe { CStr::from_ptr(name_ptr) };
      let name = c_name_string.to_string_lossy().into_owned();
      let value = self.get_property(&name).unwrap_or_default();
      attributes.insert(name, value);
      current_prop = xmlNextPropertySibling(current_prop);
    }

    attributes
  }

  /// Get a copy of this node's attributes and their namespaces
  ///
  /// The namespaces do not keep the document alive, and must not outlive it: see [`Namespace`]'s known caveat.
  pub fn get_properties_ns(self) -> HashMap<(String, Option<Namespace>), String> {
    let mut attributes = HashMap::new();

    let mut current_prop = xmlGetFirstProperty(self.0);
    while !current_prop.is_null() {
      let name_ptr = xmlAttrName(current_prop);
      if name_ptr.is_null() {
        // A property with a NULL name can't be keyed; skip it rather than
        // `strlen(NULL)` inside `CStr::from_ptr`.
        current_prop = xmlNextPropertySibling(current_prop);
        continue;
      }
      let c_name_string = unsafe { CStr::from_ptr(name_ptr) };
      let name = c_name_string.to_string_lossy().into_owned();
      let ns_ptr = xmlAttrNs(current_prop);
      if ns_ptr.is_null() {
        let value = self.get_property_no_ns(&name).unwrap_or_default();
        attributes.insert((name, None), value);
      } else {
        let ns = Namespace::of(ns_ptr, xmlGetDoc(self.0), None);
        let value = self
          .get_property_ns(&name, &ns.get_href())
          .unwrap_or_default();
        attributes.insert((name, Some(ns)), value);
      }
      current_prop = xmlNextPropertySibling(current_prop);
    }

    attributes
  }

  /// Alias for `get_properties`
  pub fn get_attributes(self) -> HashMap<String, String> {
    self.get_properties()
  }

  /// Alias for `get_properties_ns`
  pub fn get_attributes_ns(self) -> HashMap<(String, Option<Namespace>), String> {
    self.get_properties_ns()
  }

  /// Check if a property has been defined, without allocating its value
  pub fn has_property(self, name: &str) -> bool {
    let value_ptr = with_c_str_or_else(
      name,
      |_| ptr::null_mut(),
      |c_name| unsafe { xmlHasProp(self.0, c_name) },
    );
    !value_ptr.is_null()
  }

  /// Check if property `name` in namespace `ns` exists
  pub fn has_property_ns(self, name: &str, ns: &str) -> bool {
    let value_ptr = with_c_strs_or_else(
      name,
      ns,
      |_| ptr::null_mut(),
      |c_name, c_ns| unsafe { xmlHasNsProp(self.0, c_name, c_ns) },
    );
    !value_ptr.is_null()
  }

  /// Check if property `name` with no namespace exists
  pub fn has_property_no_ns(self, name: &str) -> bool {
    let value_ptr = with_c_str_or_else(
      name,
      |_| ptr::null_mut(),
      |c_name| unsafe { xmlHasNsProp(self.0, c_name, ptr::null()) },
    );
    !value_ptr.is_null()
  }

  /// Alias for has_property
  pub fn has_attribute(self, name: &str) -> bool {
    self.has_property(name)
  }

  /// Alias for has_property_ns
  pub fn has_attribute_ns(self, name: &str, ns: &str) -> bool {
    self.has_property_ns(name, ns)
  }

  /// Alias for has_property_no_ns
  pub fn has_attribute_no_ns(self, name: &str) -> bool {
    self.has_property_no_ns(name)
  }

  /// Gets the active namespace associated of this node
  ///
  /// The namespaces do not keep the document alive, and must not outlive it: see [`Namespace`]'s known caveat.
  pub fn get_namespace(self) -> Option<Namespace> {
    let ns_ptr = xmlNodeNs(self.0);
    if ns_ptr.is_null() {
      None
    } else {
      Some(Namespace::of(ns_ptr, xmlGetDoc(self.0), None))
    }
  }

  /// Gets a list of namespaces associated with this node
  ///
  /// The namespaces keep `doc` alive, as a `Node`'s do, when it is this node's document.
  pub fn get_namespaces(self, doc: &Document) -> Vec<Namespace> {
    let list_ptr_raw = unsafe { xmlGetNsList(doc.doc_ptr(), self.0) };
    if list_ptr_raw.is_null() {
      Vec::new()
    } else {
      let mut namespaces = Vec::new();
      // The namespaces are this node's document's, which may not be `doc`.
      let doc_ptr = xmlGetDoc(self.0);
      let document = (doc_ptr == doc.doc_ptr()).then_some(&doc.0);
      let mut ptr_iter = list_ptr_raw as *mut xmlNsPtr;
      unsafe {
        while !ptr_iter.is_null() && !(*ptr_iter).is_null() {
          namespaces.push(Namespace::of(*ptr_iter, doc_ptr, document.cloned()));
          ptr_iter = ptr_iter.add(1);
        }
      }
      // The array is the caller's; the namespaces it points at belong to the document.
      crate::c_helpers::bindgenFree(list_ptr_raw as *mut std::os::raw::c_void);
      namespaces
    }
  }

  /// Get a list of namespaces declared with this node
  ///
  /// The namespaces do not keep the document alive, and must not outlive it: see [`Namespace`]'s known caveat.
  pub fn get_namespace_declarations(self) -> Vec<Namespace> {
    if self.get_type() != Some(NodeType::ElementNode) {
      // only element nodes can have declarations
      return Vec::new();
    }
    let mut namespaces = Vec::new();
    let mut ns_ptr = xmlNodeNsDeclarations(self.0);
    while !ns_ptr.is_null() {
      if !xmlNsPrefix(ns_ptr).is_null() || !xmlNsHref(ns_ptr).is_null() {
        namespaces.push(Namespace::of(ns_ptr, xmlGetDoc(self.0), None));
      }
      ns_ptr = xmlNextNsSibling(ns_ptr);
    }
    namespaces
  }

  /// Looks up the prefix of a namespace from its URI, basedo around a given `Node`
  pub fn lookup_namespace_prefix(self, href: &str) -> Option<String> {
    if href.is_empty() {
      return None;
    }
    let ns_ptr = with_c_str_or_else(
      href,
      |_| ptr::null_mut(),
      |c_href| unsafe { xmlSearchNsByHref(xmlGetDoc(self.0), self.0, c_href) },
    );
    if !ns_ptr.is_null() {
      Some(namespace::prefix_of(ns_ptr))
    } else {
      None
    }
  }

  /// Looks up the uri of a namespace from its prefix, basedo around a given `Node`
  pub fn lookup_namespace_uri(self, prefix: &str) -> Option<String> {
    if prefix.is_empty() {
      return None;
    }
    let ns_ptr = with_c_str_or_else(
      prefix,
      |_| ptr::null_mut(),
      |c_prefix| unsafe { xmlSearchNs(xmlGetDoc(self.0), self.0, c_prefix) },
    );
    if !ns_ptr.is_null() {
      Some(namespace::href_of(ns_ptr)).filter(|href| !href.is_empty())
    } else {
      None
    }
  }

  /// Get a set of class names from this node's attributes
  pub fn get_class_names(self) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(value) = self.get_property("class") {
      for n in value.split(' ') {
        set.insert(n.to_owned());
      }
    }
    set
  }

  /// find read-only nodes via xpath, at the specified node and a given document
  pub fn findnodes(self, xpath: &str, owner: &Document) -> Result<Vec<RoNode>, ()> {
    let context = Context::new(owner)?;
    let evaluated = context.node_evaluate_readonly(xpath, self)?;
    Ok(evaluated.get_readonly_nodes_as_vec())
  }

  /// Read-only nodes are always linked
  pub fn is_unlinked(self) -> bool {
    false
  }

  /// Read-only nodes are always document-owned.
  ///
  /// Mirrors `Node::is_rust_owned` for API uniformity. `RoNode` is a
  /// `Copy` borrow into a document tree owned by a parent `Document`;
  /// it has no `Drop` and cannot take ownership of the C allocation,
  /// so this is unconditionally `false`.
  pub fn is_rust_owned(self) -> bool {
    false
  }
  /// Read-only nodes only need a null check
  fn ptr_as_option(self, node_ptr: xmlNodePtr) -> Option<RoNode> {
    if node_ptr.is_null() {
      None
    } else {
      Some(RoNode(node_ptr))
    }
  }

  /// `libc::c_void` isn't hashable and cannot be made hashable
  pub fn to_hashable(self) -> usize {
    self.0 as usize
  }
  /// Create a mock node, used for a placeholder argument
  pub fn null() -> Self {
    RoNode(ptr::null_mut())
  }
}

/// Pass `f` a copy (by `with_str_copy`) of the libxml2 string `ptr`, NULL as `""`, so that `f` may change the document.
/// Copying a short name costs less than asking the document's dictionary whether it owns it (`xmlDictOwns`).
///
/// # Safety
/// `ptr` is NULL or a live NUL-terminated string.
unsafe fn with_libxml_str<R>(ptr: *const c_char, f: impl FnOnce(&str) -> R) -> R {
  if ptr.is_null() {
    f("")
  } else {
    with_str_copy(unsafe { CStr::from_ptr(ptr) }, f)
  }
}

/// Whether `attr` is one of `element`'s properties, compared by address (`attr` itself is not read).
fn holds_property(element: xmlNodePtr, attr: xmlAttrPtr) -> bool {
  let mut prop = xmlGetFirstProperty(element);
  while !prop.is_null() {
    if prop == attr {
      return true;
    }
    prop = xmlNextPropertySibling(prop);
  }
  false
}

impl RoNode {
  /// Calls `f` with a copy of this node's name (`""` if none); `f` may change the document.
  pub fn with_name<R>(self, f: impl FnOnce(&str) -> R) -> R {
    unsafe { with_libxml_str(xmlNodeGetName(self.0), f) }
  }

  /// Calls `f` with `get_content()`'s value, copied on the stack when short for text, CDATA, comment and PI nodes; `f`
  /// may change the document.
  pub fn with_content<R>(self, f: impl FnOnce(&str) -> R) -> R {
    let in_node = !self.0.is_null() && {
      let node_type = unsafe { (*self.0).type_ };
      node_type == xmlElementType_XML_TEXT_NODE
        || node_type == xmlElementType_XML_CDATA_SECTION_NODE
        || node_type == xmlElementType_XML_COMMENT_NODE
        || node_type == xmlElementType_XML_PI_NODE
    };
    let content = if in_node {
      unsafe { (*self.0).content as *const c_char }
    } else {
      ptr::null()
    };
    if content.is_null() {
      // (`xmlNodeGetContent` copies exactly the in-node content for these four types, NULL as NULL)
      f(&self.get_content())
    } else {
      with_str_copy(unsafe { CStr::from_ptr(content) }, f)
    }
  }

  /// Calls `f` with the namespace prefix: `None` without a namespace, `Some("")` for a default namespace; `f` may
  /// change the document.
  pub fn with_namespace_prefix<R>(self, f: impl FnOnce(Option<&str>) -> R) -> R {
    let ns_ptr = xmlNodeNs(self.0);
    if ns_ptr.is_null() {
      f(None)
    } else {
      unsafe { with_libxml_str(xmlNsPrefix(ns_ptr), |prefix| f(Some(prefix))) }
    }
  }

  /// Calls `f` with the namespace URI; `None` without a namespace; `f` may change the document.
  pub fn with_namespace_href<R>(self, f: impl FnOnce(Option<&str>) -> R) -> R {
    let ns_ptr = xmlNodeNs(self.0);
    if ns_ptr.is_null() {
      f(None)
    } else {
      unsafe { with_libxml_str(xmlNsHref(ns_ptr), |href| f(Some(href))) }
    }
  }

  /// Calls `f(name, value)` for each property in document order; nothing on a non-element. Unlike `get_properties`,
  /// it reports every same-named property from different namespaces. `f` may change the document, but not free this
  /// node. After each call the element's property list is searched again (for the property `f` may have removed), so
  /// the cost grows with the square of the property count.
  pub fn for_each_property(self, mut f: impl FnMut(&str, &str)) {
    walk_properties(|| self.0, false, |name, _, value| f(name, value))
  }

  /// As `for_each_property`, also passing the namespace URI (`None` if none).
  pub fn for_each_property_ns(self, f: impl FnMut(&str, Option<&str>, &str)) {
    walk_properties(|| self.0, true, f)
  }

  /// Alias for [`RoNode::for_each_property`]
  pub fn for_each_attribute(self, f: impl FnMut(&str, &str)) {
    self.for_each_property(f)
  }

  /// Alias for [`RoNode::for_each_property_ns`]
  pub fn for_each_attribute_ns(self, f: impl FnMut(&str, Option<&str>, &str)) {
    self.for_each_property_ns(f)
  }
}

/// The loop of `for_each_property(_ns)`, on `RoNode` and `Node`, over the properties of the element `element` returns,
/// read again after each call of `f`: NULL stops the walk. Reads each namespace URI only `with_ns`.
pub(crate) fn walk_properties(
  element: impl Fn() -> xmlNodePtr,
  with_ns: bool,
  mut f: impl FnMut(&str, Option<&str>, &str),
) {
  let mut prop = xmlGetFirstProperty(element());
  while !prop.is_null() {
    let next = xmlNextPropertySibling(prop);
    let name_ptr = xmlAttrName(prop);
    // (a property with a NULL name is skipped, as `get_properties` skips it)
    if !name_ptr.is_null() {
      let ns_ptr = if with_ns {
        xmlAttrNs(prop)
      } else {
        ptr::null_mut()
      };
      let mut with_value = |name: &str, href: Option<&str>| match unsafe { single_text_value(prop) }
      {
        Some(content) => with_str_copy(content, |value| f(name, href, value)),
        None => f(name, href, &attr_node_value(prop)),
      };
      unsafe {
        with_libxml_str(name_ptr, |name| {
          if ns_ptr.is_null() {
            with_value(name, None)
          } else {
            with_libxml_str(xmlNsHref(ns_ptr), |href| with_value(name, Some(href)))
          }
        })
      };
      // `f` may have removed `prop` or `next`, or freed the element (a `Node`'s, which its free neutralizes): go on
      // from whichever the element still holds, else stop.
      let element = element();
      if element.is_null() {
        break;
      }
      if holds_property(element, prop) {
        prop = xmlNextPropertySibling(prop);
        continue;
      }
      if !holds_property(element, next) {
        break;
      }
    }
    prop = next;
  }
}
