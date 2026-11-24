//! Node, and related, feature set
//!
use libc::{c_char, c_int, c_void};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::ffi::{CStr, CString, NulError};
use std::hash::{Hash, Hasher};
use std::ptr;
use std::rc::{Rc, Weak};
use std::str;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bindings::*;
use crate::c_helpers::*;
use crate::readonly::RoNode;
use crate::readonly::walk_properties;
use crate::tree::document::node_sets_live;
use crate::tree::namespace::{self, Namespace};
use crate::tree::nodetype::NodeType;
use crate::tree::{Document, DocumentRef, DocumentWeak};
use crate::xpath::Context;

/// Deprecated, unused. Formerly the `Rc::strong_count` threshold consulted by
/// [`Node::node_ptr_mut`]. That heuristic was replaced by a
/// `RefCell::try_borrow_mut` check (a high clone count is not an aliasing
/// conflict), so this value is no longer read. Retained only for API
/// compatibility.
#[deprecated(note = "no longer used; node_ptr_mut now relies on RefCell::try_borrow_mut")]
pub static mut NODE_RC_MAX_GUARD: usize = 2;

/// Deprecated no-op. The strong-count guard it tuned was removed in favor of a
/// `RefCell::try_borrow_mut` check in [`Node::node_ptr_mut`]; calling this has
/// no effect. Retained so existing callers continue to compile.
#[deprecated(note = "no-op: node_ptr_mut now relies on RefCell::try_borrow_mut")]
pub fn set_node_rc_guard(_value: usize) {}

pub(crate) type NodeRef = Rc<RefCell<_Node>>;
/// A wrapper held weakly, by the document while its node is unlinked.
pub(crate) type NodeWeak = Weak<RefCell<_Node>>;

/// Lifecycle state of a wrapped libxml2 node.
///
/// The three variants correspond to disjoint lifetime-ownership regimes
/// that drive `_Node`'s `Drop` behavior:
///
/// * `Linked` — the node is attached to its document tree. The owning
///   document's `xmlFreeDoc` will reclaim it; the wrapper must not free.
/// * `Unlinked` — `xmlUnlinkNode` has detached the node from its
///   parent/siblings, but `node->doc` still points at the source xmlDoc.
///   The source still owns the C allocation. The wrapper must not free
///   unless `node->doc` itself is NULL (a true C-level orphan).
/// * `RustOwned` — the caller has explicitly transferred ownership to
///   the Rust wrapper via `Node::set_rust_owned`. The C allocation will
///   be freed via `xmlFreeNode` when the last `Node` clone drops,
///   regardless of `node->doc`.
///
/// The state-space is intentionally smaller than `(unlinked: bool,
/// rust_owned: bool)`: `(Linked, RustOwned)` is meaningless, so we make
/// it unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Linkage {
  Linked,
  Unlinked,
  RustOwned,
}

#[derive(Debug)]
pub(crate) struct _Node {
  /// libxml's xmlNodePtr
  node_ptr: xmlNodePtr,
  /// Reference to parent `Document`
  document: DocumentWeak,
  /// Lifecycle state — see `Linkage` for the semantics of each variant.
  linkage: Linkage,
  /// Whether `unlink_node` unlinked the node from its tree (until it is linked again): `set_rust_owned` then leaves
  /// its lifetime to the last clone, as the document no longer holds the wrapper strongly (see `_Document::rust_owned`).
  /// A node created unlinked (`Node::new`) stays cached, rust-owned or not, until its document drops.
  unlinked_from_tree: bool,
}

/// An xml node
#[derive(Clone, Debug)]
pub struct Node(pub(crate) NodeRef);

impl Hash for Node {
  /// Generates a hash value from the `node_ptr` value.
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.node_ptr().hash(state);
  }
}

impl PartialEq for Node {
  /// Two nodes are considered equal, if they point to the same xmlNode.
  fn eq(&self, other: &Node) -> bool {
    std::ptr::eq(self.node_ptr(), other.node_ptr())
  }
}

impl Eq for Node {}

impl Drop for _Node {
  /// Free the C node if and only if it has been detached from any
  /// libxml2 document. The historical heuristic — "free if the wrapper
  /// is `unlinked`" — conflates two distinct states:
  ///   * tree-detached (parent==NULL, but doc still points at the
  ///     source xmlDoc and that xmlDoc still owns the allocation), and
  ///   * Rust-owned (no document references the node; the wrapper
  ///     itself must call xmlFreeNode or memory leaks).
  ///
  /// Detaching a node from its parent (xmlUnlinkNode) does NOT clear
  /// node->doc — the document still owns the underlying memory and its
  /// xmlFreeDoc will reclaim it. When the wrapper drops, firing
  /// xmlFreeNode in that situation is a use-after-free in waiting:
  /// any other Rust wrapper or C-side reference to the same xmlNodePtr
  /// (a sibling's prev/next, the document's idtable, an ID lookup that
  /// hasn't run yet) sees freed memory.
  ///
  /// We now key the free on the C-level node->doc field: if it's NULL
  /// the node is truly orphan and we own it; otherwise the source
  /// document is the lifetime owner. This matches what xmlAddChild /
  /// xmlDocSetRootElement / xmlSetTreeDoc actually do — they install a
  /// doc pointer to claim ownership, and xmlUnlinkNode preserves that
  /// pointer because it only severs sibling/parent links.
  ///
  /// Real-world driver: a host application that detaches sibling
  /// subtrees from a still-live source document, copies them out via
  /// xmlCopyNode, and then drops the wrapper for each detached
  /// subtree. Under the old `unlinked`-based rule, every wrapper drop
  /// fired xmlFreeNode against memory that the source doc still
  /// considered live, corrupting its dictionary and causing the next
  /// xmlCopyNode against a sibling to return NULL.
  fn drop(&mut self) {
    let node_ptr = self.node_ptr;
    if node_ptr.is_null() {
      return;
    }
    match self.linkage {
      // Source document owns the allocation; `xmlFreeDoc` will reclaim
      // it. Firing `xmlFreeNode` here would be a use-after-free in
      // waiting (sibling/parent links, ID table, prev clones).
      Linkage::Linked => {}
      // Detached, but `node->doc` still points at the source. Free only
      // if `node->doc` is NULL — a true C-level orphan no document will
      // reach via tree walks.
      Linkage::Unlinked => {
        let still_doc_owned = unsafe {
          let n: *const crate::bindings::_xmlNode = node_ptr as *const _;
          !(*n).doc.is_null()
        };
        if !still_doc_owned {
          self.free_node();
        }
      }
      // Caller took ownership; the wrapper is the sole lifetime owner.
      Linkage::RustOwned => self.free_node(),
    }
  }
}

impl _Node {
  /// `xmlFreeNode` on this wrapper's node, which frees its whole subtree, after neutralizing the cached wrappers of
  /// every node it frees (#232): a wrapper of a descendant, or of the node itself wrapped afresh after an unlink,
  /// reads as `Node::null()` instead of freed memory. Neutralizing is skipped when the document is gone (its drop
  /// clears the cache before `xmlFreeDoc`) or mutably borrowed.
  fn free_node(&self) {
    let doc = self.document.upgrade();
    let mut doc = doc.as_ref().and_then(|doc| doc.try_borrow_mut().ok());
    if xmlGetNodeType(self.node_ptr) == xmlElementType_XML_DTD_NODE {
      // Its document's entity references may point into it: the document keeps it, and frees it (#235).
      if let Some(doc) = doc.as_mut() {
        doc.keep_dtd(self.node_ptr as xmlDtdPtr);
      }
      return;
    }
    if let Some(doc) = doc.as_mut() {
      doc.prepare_to_free(self.node_ptr);
    }
    unsafe { xmlFreeNode(self.node_ptr) };
  }
}

/// Whether a node of type `node_type` is an element, attribute or entity declaration, which its DTD frees through its
/// tables (`xmlFreeDtd`), and the document's entity references point at: `unlink_node` and `free_subtree` leave one
/// alone, and linking one is refused (#235).
fn is_declaration(node_type: &Option<NodeType>) -> bool {
  matches!(
    node_type,
    Some(NodeType::ElementDecl | NodeType::AttributeDecl | NodeType::EntityDecl)
  )
}

/// Calls `f` on every node `xmlFreeNode(top)` frees, `top` first: its attributes (of an element), its children and
/// their subtrees, but not the children of an entity reference, which belong to the entity's declaration. `f` may
/// forget a node, but neither free it nor change the tree: the walk reads the links of the nodes it has passed. It
/// follows those links (down, across, back up by `parent`) rather than keep a stack, so it allocates nothing and
/// takes any depth.
pub(crate) fn for_each_freed_with(top: xmlNodePtr, mut f: impl FnMut(xmlNodePtr)) {
  if top.is_null() {
    return;
  }
  let mut node = top;
  loop {
    // Only elements (and XInclude's start/end copies of one) have a `properties` field; `xmlGetFirstProperty`
    // answers NULL through any other node, where the slot is out of bounds or holds other data.
    let attr = xmlGetFirstProperty(node) as xmlNodePtr;
    let child = if xmlGetNodeType(node) == xmlElementType_XML_ENTITY_REF_NODE {
      ptr::null_mut()
    } else {
      xmlGetFirstChild(node)
    };
    f(node);
    if !attr.is_null() {
      node = attr;
      continue;
    }
    if !child.is_null() {
      node = child;
      continue;
    }
    // Back up to the nearest node, short of `top`, with a next sibling; past an element's last attribute, its
    // children come next.
    loop {
      if node == top {
        return;
      }
      let is_attr = xmlGetNodeType(node) == xmlElementType_XML_ATTRIBUTE_NODE;
      let next = if is_attr {
        xmlNextPropertySibling(node as xmlAttrPtr) as xmlNodePtr
      } else {
        xmlNextSibling(node)
      };
      if !next.is_null() {
        node = next;
        break;
      }
      let parent = xmlGetParent(node);
      if parent.is_null() {
        return; // Not in `top`'s subtree after all.
      }
      if is_attr {
        let child = xmlGetFirstChild(parent);
        if !child.is_null() {
          node = child;
          break;
        }
      }
      node = parent;
    }
  }
}

/// The nodes a libxml2 insertion of `new` (`xmlAddChild`, `xmlAddPrevSibling`, `xmlAddNextSibling`) may free,
/// collected while they are alive, so that the wrappers of those it did free can be neutralized after it (#234):
/// * `new`, which libxml2 merges into a text node and frees, returning that node instead of `new`: a text node added
///   next to another (by `xmlAddChild`, and by the sibling functions before libxml2 2.13), and any node added as a
///   child of a text node, subtree and all.
/// * the attribute of the same name that an attribute `new` replaces.
struct InsertionCasualties {
  new: xmlNodePtr,
  /// The element whose attribute an attribute `new` may replace.
  element: xmlNodePtr,
  /// `new`'s descendants (and attributes), when libxml2 may merge `new` away.
  merged_descendants: Option<Vec<xmlNodePtr>>,
  /// Each attribute of `element` named like the attribute `new`, then the rest of its subtree.
  replaced: Vec<Vec<xmlNodePtr>>,
  /// The namespace declarations of `new` and its descendants, with the element that declared each list, taken off
  /// before `new` is added under a text node, which merges `new` away and would free them with it: a `Namespace`, or
  /// the namespace of a node elsewhere, may still point at one (#235).
  declarations: Vec<(xmlNodePtr, xmlNsPtr)>,
}

impl InsertionCasualties {
  /// For `new` inserted as a child of `parent` (`as_child`), or as a sibling of one of `parent`'s children; `None`
  /// when the insertion frees nothing, the usual case: `new` is neither a text node nor an attribute, and is not added
  /// as a text node's child.
  #[inline(always)]
  fn collect(new: xmlNodePtr, parent: xmlNodePtr, as_child: bool) -> Option<Self> {
    let new_type = xmlGetNodeType(new);
    note_wrapped(new_type, parent);
    let under_text = as_child && xmlGetNodeType(parent) == xmlElementType_XML_TEXT_NODE;
    if under_text
      || new_type == xmlElementType_XML_TEXT_NODE
      || new_type == xmlElementType_XML_ATTRIBUTE_NODE
    {
      Some(Self::collect_freeable(new, new_type, parent, under_text))
    } else {
      None
    }
  }

  /// [`collect`](Self::collect), when the insertion may free nodes; out of line, so that the usual insertion stays
  /// small.
  #[inline(never)]
  fn collect_freeable(
    new: xmlNodePtr,
    new_type: xmlElementType,
    parent: xmlNodePtr,
    under_text: bool,
  ) -> Self {
    let mut declarations = Vec::new();
    let merged_descendants = if under_text {
      let mut descendants = Vec::new();
      for_each_freed_with(new, |node_ptr| {
        if node_ptr != new {
          descendants.push(node_ptr)
        }
        let declared = xmlNodeTakeNsDeclarations(node_ptr);
        if !declared.is_null() {
          declarations.push((node_ptr, declared));
        }
      });
      Some(descendants)
    } else if new_type == xmlElementType_XML_TEXT_NODE {
      Some(Vec::new()) // A text node has none, and the empty `Vec` does not allocate.
    } else {
      None
    };
    let mut replaced = Vec::new();
    if new_type == xmlElementType_XML_ATTRIBUTE_NODE {
      let name = xmlNodeGetName(new) as *const xmlChar;
      let mut attr = xmlGetFirstProperty(parent);
      while !attr.is_null() {
        // SAFETY: `attr` is a live attribute of `parent`; `xmlStrEqual` accepts NULL names.
        if attr as xmlNodePtr != new && unsafe { xmlStrEqual((*attr).name, name) } != 0 {
          let mut subtree = Vec::new();
          for_each_freed_with(attr as xmlNodePtr, |node_ptr| subtree.push(node_ptr));
          replaced.push(subtree);
        }
        attr = xmlNextPropertySibling(attr);
      }
    }
    InsertionCasualties {
      new,
      element: parent,
      merged_descendants,
      replaced,
      declarations,
    }
  }

  /// Neutralizes the wrappers of the collected nodes libxml2 freed, given the node it `inserted`: `new` with its
  /// subtree, if libxml2 returned another node in its place, and each replaced attribute, with its subtree, that is no
  /// longer `element`'s. Reads none of them; nothing was freed when the insertion failed. The cached wrappers are
  /// neutralized in the documents of both `wrappers`, the nodes of the insertion, and so are the `wrappers`
  /// themselves, which a cache may not hold. The namespace declarations taken off `new`'s subtree are kept by the
  /// document, if it was merged away, or given back.
  fn neutralize_freed(self, inserted: xmlNodePtr, wrappers: [&Node; 2]) {
    let merged_away = !inserted.is_null() && inserted != self.new;
    if !merged_away {
      for &(declarer, declared) in &self.declarations {
        // SAFETY: `declarer` is a live element, whose declarations were taken off it by `collect_freeable`.
        unsafe { (*declarer).nsDef = declared };
      }
    }
    if inserted.is_null() {
      return;
    }
    let merged = if merged_away {
      self.merged_descendants
    } else {
      None
    };
    let mut replaced = self.replaced;
    if !replaced.is_empty() {
      replaced.retain(|subtree| !has_attribute(self.element, subtree[0]));
    }
    if merged.is_none() && replaced.is_empty() {
      return;
    }
    let [first, second] = wrappers.map(|wrapper| wrapper.upgrade_docref());
    let second = second.filter(|second| {
      !first
        .as_ref()
        .is_some_and(|first| Rc::ptr_eq(first, second))
    });
    let mut docs = [first.as_ref(), second.as_ref()].map(|doc| doc.map(|doc| doc.borrow_mut()));
    let mut neutralize = |node_ptr: xmlNodePtr| {
      for wrapper in wrappers {
        if wrapper.node_ptr() == node_ptr {
          wrapper.null_out();
        }
      }
      for doc in docs.iter_mut().flatten() {
        doc.neutralize(node_ptr);
      }
    };
    if let Some(descendants) = merged {
      neutralize(self.new);
      descendants.into_iter().for_each(&mut neutralize);
    }
    replaced.into_iter().flatten().for_each(neutralize);
    if merged_away && let Some(doc) = docs.iter_mut().flatten().next() {
      for (_, declared) in self.declarations {
        doc.keep_declarations(declared);
      }
    }
  }
}

/// Whether a wrapper of an attribute may be cached, in any document; see [`note_wrapped`]. Until then, the
/// `remove_property` methods skip looking for wrappers of the attribute they free, unless [`VALUE_NODE_WRAPPED`].
static ATTRIBUTE_WRAPPED: AtomicBool = AtomicBool::new(false);

/// Whether a wrapper of an attribute's value node (a child of an attribute) may be cached, in any document; see
/// [`note_wrapped`]. Until then, `set_property` and `set_property_ns` skip looking for wrappers of the value nodes they
/// free ([`ValueNodes`]).
static VALUE_NODE_WRAPPED: AtomicBool = AtomicBool::new(false);

/// Notes, for [`ATTRIBUTE_WRAPPED`] and [`VALUE_NODE_WRAPPED`], that a wrapper's node, of type `node_type`, is or is
/// about to be a child of `parent`. Called when a node is wrapped (`_Document::insert_node`) and when a node is
/// inserted (`add_child`, `add_prev_sibling`, `add_next_sibling`, through `InsertionCasualties::collect`); every way a
/// cached wrapper's node becomes an attribute's, or comes under one, must call it. The flags are never cleared: most
/// programs never wrap these nodes, and those that do pay for the search.
pub(crate) fn note_wrapped(node_type: xmlElementType, parent: xmlNodePtr) {
  // A document and its wrappers stay on one thread, so a later call on it sees these stores.
  if node_type == xmlElementType_XML_ATTRIBUTE_NODE {
    ATTRIBUTE_WRAPPED.store(true, Ordering::Relaxed);
  }
  if xmlGetNodeType(parent) == xmlElementType_XML_ATTRIBUTE_NODE {
    VALUE_NODE_WRAPPED.store(true, Ordering::Relaxed);
  }
}

/// The cached wrappers in the value nodes of the attributes a setter may modify (all the value nodes, while the document
/// logs frees for a live XPath node set), collected while they are alive:
/// `xmlSetProp` and `xmlSetNsProp` free the old value nodes of the attribute they modify, and keep the attribute.
/// Which attribute that is, libxml2 decides (it resolves a prefixed name to its namespace), so every attribute with a
/// candidate name is collected, and [`neutralize_replaced`](Self::neutralize_replaced) neutralizes the value nodes
/// libxml2 did take out.
struct ValueNodes {
  /// A candidate attribute, one of its value nodes, and the cached wrappers in that value node's subtree.
  cached: Vec<(xmlNodePtr, xmlNodePtr, Vec<xmlNodePtr>)>,
}

impl ValueNodes {
  /// Collects from the attributes of `element` named `name`, `c_name` as a C string, or, when `local_too` (for
  /// `xmlSetProp`, which resolves a prefixed name to its local part in the prefix's namespace, if the prefix is bound),
  /// its local part; `None` when no candidate value node has a cached wrapper.
  fn collect(element: &Node, name: &str, c_name: *const xmlChar, local_too: bool) -> Option<Self> {
    let colon = if local_too {
      name.bytes().position(|byte| byte == b':')
    } else {
      None
    };
    // SAFETY: the local part is the rest of `c_name`, NUL-terminated with it.
    let names = [
      c_name,
      colon.map_or(ptr::null(), |colon| unsafe { c_name.add(colon + 1) }),
    ];
    let mut cached = Vec::new();
    let mut doc = None;
    let mut attr = xmlGetFirstProperty(element.node_ptr());
    while !attr.is_null() {
      let mut value = xmlGetFirstChild(attr as xmlNodePtr);
      // SAFETY: `attr` is a live attribute of `element`; its name and each of `names` is a C string, if not NULL.
      if !value.is_null()
        && !unsafe { (*attr).name }.is_null()
        && (names.iter()).any(|&name| !name.is_null() && unsafe { c_str_eq((*attr).name, name) })
      {
        if doc.is_none() {
          doc = element.upgrade_docref();
        }
        let Some(doc) = &doc else {
          break;
        };
        let doc = doc.borrow();
        let logs_frees = doc.logs_frees();
        while !value.is_null() {
          let mut wrapped = Vec::new();
          for_each_freed_with(value, |node_ptr| {
            if logs_frees || doc.is_cached(node_ptr) {
              wrapped.push(node_ptr);
            }
          });
          if !wrapped.is_empty() {
            cached.push((attr as xmlNodePtr, value, wrapped));
          }
          value = xmlNextSibling(value);
        }
      }
      attr = xmlNextPropertySibling(attr);
    }
    (!cached.is_empty()).then_some(ValueNodes { cached })
  }

  /// Neutralizes the collected wrappers of each value node that is no longer its attribute's: libxml2 freed it.
  fn neutralize_replaced(self, element: &Node) {
    let Some(doc) = element.upgrade_docref() else {
      return;
    };
    let mut doc = doc.borrow_mut();
    for (attr, value, wrapped) in self.cached {
      if !has_child(attr, value) {
        for node_ptr in wrapped {
          doc.neutralize(node_ptr);
        }
      }
    }
  }
}

/// Whether the C strings `a` and `b` are equal: `xmlStrEqual`, compared in place rather than through a call.
///
/// # Safety
/// Both are live and NUL-terminated.
unsafe fn c_str_eq(a: *const xmlChar, b: *const xmlChar) -> bool {
  let mut i = 0;
  loop {
    // SAFETY: neither string ends before `i`, so both extend to it.
    let (x, y) = unsafe { (*a.add(i), *b.add(i)) };
    if x != y {
      return false;
    }
    if x == 0 {
      return true;
    }
    i += 1;
  }
}

/// Whether `node` is one of `parent`'s children. Compares pointers only: `node` may be freed.
fn has_child(parent: xmlNodePtr, node: xmlNodePtr) -> bool {
  let mut current = xmlGetFirstChild(parent);
  while !current.is_null() {
    if current == node {
      return true;
    }
    current = xmlNextSibling(current);
  }
  false
}

/// Whether `attr` is one of `element`'s attributes. Compares pointers only: `attr` may be freed.
fn has_attribute(element: xmlNodePtr, attr: xmlNodePtr) -> bool {
  let mut current = xmlGetFirstProperty(element);
  while !current.is_null() {
    if current as xmlNodePtr == attr {
      return true;
    }
    current = xmlNextPropertySibling(current);
  }
  false
}

/// Whether a node of `node_type` is a document's own node (an HTML one too) or a document fragment: no node of a tree,
/// it is never unlinked, or freed as one.
fn is_document(node_type: &Option<NodeType>) -> bool {
  matches!(
    node_type,
    Some(NodeType::DocumentNode | NodeType::HtmlDocumentNode | NodeType::DocumentFragNode)
  )
}

/// Why a node of type `node_type` may never be linked, if it may not: a document's own node is no node of a tree.
/// libxml2 frees one added under a text node as an ordinary node, and `xmlFreeDoc` recurses through one linked
/// elsewhere. A DTD, and a declaration in one, is its document's to free (see [`Node::unlink_node`]): libxml2 frees one
/// added under a text node at once, never a DTD linked as a child, and a declaration through its DTD's tables too.
#[inline]
pub(crate) fn type_refusal(node_type: xmlElementType) -> Option<&'static str> {
  if node_type == xmlElementType_XML_DOCUMENT_NODE
    || node_type == xmlElementType_XML_HTML_DOCUMENT_NODE
  {
    Some("a document cannot be linked into a tree")
  } else if (xmlElementType_XML_DTD_NODE..=xmlElementType_XML_ENTITY_DECL).contains(&node_type) {
    Some("a DTD, or a declaration in one, cannot be linked: it is its document's to free")
  } else {
    None
  }
}

/// Whether `node` is `ancestor`, or one of its descendants.
#[inline(never)]
fn is_within(mut node: xmlNodePtr, ancestor: xmlNodePtr) -> bool {
  while !node.is_null() {
    if node == ancestor {
      return true;
    }
    node = xmlGetParent(node);
  }
  false
}

/// Whether any node is under `node`, of type `node_type`: a child, or an attribute.
#[inline]
fn has_descendants(node: xmlNodePtr, node_type: xmlElementType) -> bool {
  // SAFETY: an element-like node is a live `xmlNode`, whose attributes `properties` holds.
  !xmlGetFirstChild(node).is_null()
    || is_element_like(node_type) && !unsafe { (*node).properties }.is_null()
}

/// Why `new` may not be linked next to `anchor`, whose parent is `parent`, if it may not, after [`Node::link_refusal`]:
/// a sibling of a node without a parent is in no tree, which would free it (`xmlFreeNode` frees a node, not its
/// siblings), and an attribute's siblings are attributes, which libxml2 checks for an attribute `new` only: it links
/// any other node into an element's attributes, which it then frees as an attribute.
#[inline]
fn sibling_refusal(
  anchor: xmlNodePtr,
  parent: xmlNodePtr,
  new: xmlNodePtr,
) -> Option<&'static str> {
  if parent.is_null() {
    Some("a node without a parent takes no sibling: nothing would free it")
  } else if (xmlGetNodeType(anchor) == xmlElementType_XML_ATTRIBUTE_NODE)
    != (xmlGetNodeType(new) == xmlElementType_XML_ATTRIBUTE_NODE)
  {
    Some("an attribute's siblings are attributes")
  } else {
    None
  }
}

impl Node {
  /// Create a new node, bound to a given document.
  pub fn new(name: &str, ns: Option<Namespace>, doc: &Document) -> Result<Self, ()> {
    // We will only allow to work with document-bound nodes for now, to avoid the problems of memory management.

    let c_name = CString::new(name).map_err(|_| ())?;
    if ns
      .as_ref()
      .is_some_and(|ns| !ns.may_be_used_in(doc.doc_ptr()))
    {
      return Err(());
    }
    // (`ns` lives until the end, and with it its document.)
    let ns_ptr = ns.as_ref().map_or(ptr::null_mut(), Namespace::ns_ptr);
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

  /// Mutably borrows the underlying libxml2 `xmlNodePtr`.
  ///
  /// Returns `Err` only when this node's `RefCell` is *actively* borrowed — a
  /// genuine re-entrant aliasing conflict on the same wrapped node. A high
  /// `Rc::strong_count` is not a conflict and does not block the borrow: clones
  /// are normal (the owning document caches one wrapper per node, and callers
  /// hold their own), and all clones share a single `RefCell`, so
  /// `try_borrow_mut` is the exact per-node exclusion check. This replaces the
  /// former `Rc::strong_count <= NODE_RC_MAX_GUARD` heuristic, which rejected
  /// benign clones and could panic (via `borrow_mut`) on the very conflict it
  /// was meant to catch.
  ///
  /// The shared-`RefCell` invariant holds across clones and repeated accessor
  /// calls, for a linked node and an unlinked one alike: while a wrapper of a
  /// node is held, its document keeps that wrapper (cached, or for a rust-owned
  /// node in a weak map), and every accessor resolves to it.
  ///
  /// This guards the Rust wrapper, not the C node: mutations through the returned
  /// raw pointer, and the C node's lifetime, remain the caller's responsibility.
  pub fn node_ptr_mut(&mut self) -> Result<xmlNodePtr, String> {
    match self.0.try_borrow_mut() {
      Ok(inner) => Ok(inner.node_ptr),
      // Do not touch `self.0` here (e.g. `get_name`): it is actively borrowed,
      // so any further borrow would panic.
      Err(_) => {
        Err("Can not mutably reference a Node that is already actively borrowed".to_string())
      }
    }
  }

  /// Wrap a libxml node ptr with a Node
  fn _wrap(node_ptr: xmlNodePtr, unlinked: bool, document: &DocumentRef) -> Node {
    // If already seen, return saved Node
    if let Some(node) = document.borrow().get_cached(node_ptr) {
      return node;
    }
    Self::wrap_uncached(node_ptr, unlinked, document)
  }
  /// [`_wrap`](Self::_wrap) of a node whose wrapper is not cached: its rust-owned wrapper, if it has one, or a new
  /// one, cached. Out of line, so that the usual case, a cached wrapper, stays small.
  #[inline(never)]
  fn wrap_uncached(node_ptr: xmlNodePtr, unlinked: bool, document: &DocumentRef) -> Node {
    if let Some(node) = document.borrow().get_rust_owned(node_ptr) {
      return node;
    }
    // If newly encountered pointer, wrap
    let node = _Node {
      node_ptr,
      document: Rc::downgrade(document),
      linkage: if unlinked {
        Linkage::Unlinked
      } else {
        Linkage::Linked
      },
      unlinked_from_tree: false,
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
    let c_content = CString::new(content).map_err(|_| ())?;
    unsafe {
      let node = xmlNewDocText(doc.doc_ptr(), c_content.as_bytes().as_ptr());
      if node.is_null() {
        Err(())
      } else {
        Ok(Node::wrap_new(node, &doc.0))
      }
    }
  }

  /// Create a new comment node, bound to a given document. The returned
  /// node is unlinked — attach it via `add_child`, `add_prev_sibling` or
  /// `add_next_sibling`.
  ///
  /// Returns `Err(())` if the C string cannot be constructed (the comment
  /// contains an embedded NUL), or if libxml2 returns NULL.
  pub fn new_comment(content: &str, doc: &Document) -> Result<Self, ()> {
    let c_content = CString::new(content).map_err(|_| ())?;
    unsafe {
      let node = xmlNewDocComment(doc.doc_ptr(), c_content.as_bytes().as_ptr());
      if node.is_null() {
        Err(())
      } else {
        Ok(Node::wrap_new(node, &doc.0))
      }
    }
  }
  /// Create a mock node, used for a placeholder argument
  pub fn mock(doc: &Document) -> Self {
    Node::new("mock", None, doc).unwrap()
  }

  /// Create a mock node, used for a placeholder argument
  pub fn null() -> Self {
    Node(Rc::new(RefCell::new(_Node {
      node_ptr: ptr::null_mut(),
      document: Rc::downgrade(&Document::null_ref()),
      linkage: Linkage::Unlinked,
      unlinked_from_tree: false,
    })))
  }

  /// `libc::c_void` isn't hashable and cannot be made hashable
  pub fn to_hashable(&self) -> usize {
    self.node_ptr() as usize
  }

  pub(crate) fn get_docref(&self) -> DocumentWeak {
    self.0.borrow().document.clone()
  }

  /// The node's document, if it is alive: `get_docref().upgrade()` without cloning the weak reference.
  pub(crate) fn upgrade_docref(&self) -> Option<DocumentRef> {
    self.0.borrow().document.upgrade()
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
          match current_node.get_next_sibling() {
            Some(sibling) => {
              current_node = sibling;
            }
            _ => {
              break;
            }
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
          match current_node.get_prev_sibling() {
            Some(sibling) => {
              current_node = sibling;
            }
            _ => {
              break;
            }
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
          match current_node.get_next_sibling() {
            Some(sibling) => {
              current_node = sibling;
            }
            _ => {
              break;
            }
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
          match current_node.get_prev_sibling() {
            Some(sibling) => {
              current_node = sibling;
            }
            _ => {
              break;
            }
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
    self.wrap_children(|_| true)
  }

  /// Returns all child elements of the given node as a vector
  pub fn get_child_elements(&self) -> Vec<Node> {
    // (only the elements are wrapped: a text child is never registered just to be filtered out)
    self.wrap_children(|child| xmlGetNodeType(child) == xmlElementType_XML_ELEMENT_NODE)
  }

  /// The children `keep` accepts, in document order, each wrapped once, against one upgrade of the document, into a
  /// vector sized by a first walk over the list.
  fn wrap_children(&self, keep: impl Fn(xmlNodePtr) -> bool) -> Vec<Node> {
    let first = xmlGetFirstChild(self.node_ptr());
    if first.is_null() {
      return Vec::new();
    }
    let mut count = 0;
    let mut child = first;
    while !child.is_null() {
      if keep(child) {
        count += 1;
      }
      child = xmlNextSibling(child);
    }
    let mut children = Vec::with_capacity(count);
    if count > 0 {
      let doc_ref = self.0.borrow().document.upgrade().unwrap();
      let mut child = first;
      while !child.is_null() {
        if keep(child) {
          children.push(Node::wrap(child, &doc_ref));
        }
        child = xmlNextSibling(child);
      }
    }
    children
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
  ///
  /// libxml2 before 2.13 merges a text `new_sibling` into an adjacent text node and frees it, and an attribute
  /// `new_sibling` replaces (frees) the attribute of the same name, which may be this one. A freed node's wrappers
  /// read as `Node::null()` afterwards.
  ///
  /// A node without a parent takes no sibling, and an attribute's siblings are attributes.
  pub fn add_prev_sibling(
    &mut self,
    new_sibling: &mut Node,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(refusal) = self.link_refusal(new_sibling) {
      return Err(From::from(refusal));
    }
    let self_ptr = self.node_ptr_mut()?;
    let new_ptr = new_sibling.node_ptr_mut()?;
    let parent = xmlGetParent(self_ptr);
    if let Some(refusal) = sibling_refusal(self_ptr, parent, new_ptr) {
      return Err(From::from(refusal));
    }
    let casualties = InsertionCasualties::collect(new_ptr, parent, false);
    let inserted = unsafe { xmlAddPrevSibling(self_ptr, new_ptr) };
    if let Some(casualties) = casualties {
      casualties.neutralize_freed(inserted, [&*self, &*new_sibling]);
    }
    new_sibling.settle_linkage(inserted);
    if inserted.is_null() {
      Err(From::from("add_prev_sibling returned NULL"))
    } else {
      Ok(())
    }
  }

  /// Add a next sibling
  ///
  /// libxml2 before 2.13 merges a text `new_sibling` into an adjacent text node and frees it, and an attribute
  /// `new_sibling` replaces (frees) the attribute of the same name, which may be this one. A freed node's wrappers
  /// read as `Node::null()` afterwards.
  ///
  /// A node without a parent takes no sibling, and an attribute's siblings are attributes.
  pub fn add_next_sibling(
    &mut self,
    new_sibling: &mut Node,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(refusal) = self.link_refusal(new_sibling) {
      return Err(From::from(refusal));
    }
    let self_ptr = self.node_ptr_mut()?;
    let new_ptr = new_sibling.node_ptr_mut()?;
    let parent = xmlGetParent(self_ptr);
    if let Some(refusal) = sibling_refusal(self_ptr, parent, new_ptr) {
      return Err(From::from(refusal));
    }
    let casualties = InsertionCasualties::collect(new_ptr, parent, false);
    let inserted = unsafe { xmlAddNextSibling(self_ptr, new_ptr) };
    if let Some(casualties) = casualties {
      casualties.neutralize_freed(inserted, [&*self, &*new_sibling]);
    }
    new_sibling.settle_linkage(inserted);
    if inserted.is_null() {
      Err(From::from("add_next_sibling returned NULL"))
    } else {
      Ok(())
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
    c_str_to_string(unsafe { CStr::from_ptr(name_ptr) })
  }

  /// Sets the name of this `Node`
  pub fn set_name(&mut self, name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name)?;
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
    let rust_utf8 = c_str_to_string(c_string);
    bindgenFree(content_ptr as *mut c_void);
    rust_utf8
  }

  /// Sets the content of this `Node`
  ///
  /// For an element, an attribute or a document fragment, libxml2 reads `content` as an attribute value is read, with
  /// entity and character references: `"a &amp; b"` and `"a &#38; b"` set the text `a & b`, and `"&name;"` adds a
  /// reference to the entity `name`. A `&` that starts no well-formed reference loses text: libxml2 drops it, or the
  /// text up to the next `;`, and before 2.13 it may empty the node and print an error. This returns `Ok` either way
  /// (#141). To set text as it is, use [`set_text_content`](Node::set_text_content). Other nodes, such as text and
  /// comments, take `content` as it is.
  ///
  /// The content of an element, attribute or document fragment replaces its children, which libxml2 frees: their
  /// wrappers, and those of their descendants, read as `Node::null()` afterwards. An entity reference is left alone,
  /// as libxml2 2.13 and later do: its child is the entity's declaration.
  pub fn set_content(&mut self, content: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_content = CString::new(content)?;
    let node_ptr = self.node_ptr_mut()?;
    let node_type = self.get_type();
    if node_type == Some(NodeType::EntityRefNode) {
      // Its child is the entity's declaration, which libxml2 before 2.13 frees here (#235); 2.13 and later leave an
      // entity reference alone, and so does this.
      return Ok(());
    }
    if matches!(
      node_type,
      Some(NodeType::ElementNode | NodeType::AttributeNode | NodeType::DocumentFragNode)
    ) && let Some(doc) = self.upgrade_docref()
    {
      let mut doc = doc.borrow_mut();
      let mut child = xmlGetFirstChild(node_ptr);
      while !child.is_null() {
        doc.prepare_to_free(child);
        child = xmlNextSibling(child);
      }
    }
    unsafe {
      xmlNodeSetContent(node_ptr, c_content.as_bytes().as_ptr());
    }
    Ok(())
  }

  /// Sets the text content of this `Node` to `text`, as it is
  ///
  /// Unlike [`set_content`](Node::set_content), this reads no `&` in `text` as a reference:
  /// `set_text_content("a & b")` sets the text `a & b` with every libxml2 version. It replaces and frees children
  /// as `set_content` does.
  pub fn set_text_content(&mut self, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let parses_references = matches!(
      self.get_type(),
      Some(NodeType::ElementNode | NodeType::AttributeNode | NodeType::DocumentFragNode)
    );
    if parses_references && text.contains('&') {
      // `&#38;` is the character reference to `&`, which libxml2 turns back into `&`.
      self.set_content(&text.replace('&', "&#38;"))
    } else {
      self.set_content(text)
    }
  }

  /// Returns the value of property `name`
  pub fn get_property(&self, name: &str) -> Option<String> {
    with_c_str_or_else(
      name,
      |_| None,
      |c_name| unsafe {
        let node_ptr = self.node_ptr();
        // (`xmlGetProp` is `xmlHasProp` and a copy of the value)
        property_value(xmlHasProp(node_ptr, c_name), || {
          xmlGetProp(node_ptr, c_name)
        })
      },
    )
  }

  /// Read property `name`, as [`Node::get_property`] does, and pass its value to `f`; unlike `get_property`, the common
  /// case allocates nothing. The name is a C string (`c"id"`), and a value that libxml2 holds as a single text or CDATA
  /// node (the case libxml2's own `xmlGetProp` also shortcuts) of at most 128 bytes is copied into a buffer on the
  /// stack. A longer value, or one of any other shape (an entity reference in the value, a DTD default), is read into a
  /// `String` as `get_property` reads it. Returns `None` when the node has no such property.
  ///
  /// `f` reads its own copy of the value, never libxml2's, so `f` may change the document, this property included.
  pub fn with_property<R>(&self, name: &CStr, f: impl FnOnce(&str) -> R) -> Option<R> {
    with_property_copy(self.node_ptr(), name, f)
  }

  /// Returns the value of property `name` in namespace `ns`
  pub fn get_property_ns(&self, name: &str, ns: &str) -> Option<String> {
    with_c_strs_or_else(
      name,
      ns,
      |_| None,
      |c_name, c_ns| unsafe {
        let node_ptr = self.node_ptr();
        // (`xmlGetNsProp` is `xmlHasNsProp` and a copy of the value)
        property_value(xmlHasNsProp(node_ptr, c_name, c_ns), || {
          xmlGetNsProp(node_ptr, c_name, c_ns)
        })
      },
    )
  }

  /// Returns the value of property `name` with no namespace
  pub fn get_property_no_ns(&self, name: &str) -> Option<String> {
    with_c_str_or_else(
      name,
      |_| None,
      |c_name| unsafe {
        let node_ptr = self.node_ptr();
        // (`xmlGetNoNsProp` is `xmlHasNsProp` with no namespace and a copy of the value)
        property_value(xmlHasNsProp(node_ptr, c_name, ptr::null()), || {
          xmlGetNoNsProp(node_ptr, c_name)
        })
      },
    )
  }

  /// Return an attribute as a `Node` struct of type AttributeNode
  pub fn get_property_node(&self, name: &str) -> Option<Node> {
    let attr_node = with_c_str_or_else(
      name,
      |_| ptr::null_mut(),
      |c_name| unsafe { xmlHasProp(self.node_ptr(), c_name) },
    );
    self.ptr_as_option(attr_node as xmlNodePtr)
  }

  /// Return an attribute in a namespace `ns` as a `Node` of type AttributeNode
  pub fn get_property_node_ns(&self, name: &str, ns: &str) -> Option<Node> {
    let attr_node = with_c_strs_or_else(
      name,
      ns,
      |_| ptr::null_mut(),
      |c_name, c_ns| unsafe { xmlHasNsProp(self.node_ptr(), c_name, c_ns) },
    );
    self.ptr_as_option(attr_node as xmlNodePtr)
  }

  /// Return an attribute with no namespace as a `Node` of type AttributeNode
  pub fn get_property_node_no_ns(&self, name: &str) -> Option<Node> {
    let attr_node = with_c_str_or_else(
      name,
      |_| ptr::null_mut(),
      |c_name| unsafe { xmlHasNsProp(self.node_ptr(), c_name, ptr::null()) },
    );
    self.ptr_as_option(attr_node as xmlNodePtr)
  }

  /// Check if a property has been defined, without allocating its value
  pub fn has_property(&self, name: &str) -> bool {
    with_c_str_or_else(
      name,
      |_| false,
      |c_name| unsafe { !xmlHasProp(self.node_ptr(), c_name).is_null() },
    )
  }

  /// Check if property `name` in namespace `ns` exists
  pub fn has_property_ns(&self, name: &str, ns: &str) -> bool {
    with_c_strs_or_else(
      name,
      ns,
      |_| false,
      |c_name, c_ns| unsafe { !xmlHasNsProp(self.node_ptr(), c_name, c_ns).is_null() },
    )
  }

  /// Check if property `name` with no namespace exists
  pub fn has_property_no_ns(&self, name: &str) -> bool {
    with_c_str_or_else(
      name,
      |_| false,
      |c_name| unsafe { !xmlHasNsProp(self.node_ptr(), c_name, ptr::null()).is_null() },
    )
  }

  /// Alias for has_property
  pub fn has_attribute(&self, name: &str) -> bool {
    self.has_property(name)
  }
  /// Alias for has_property_ns
  pub fn has_attribute_ns(&self, name: &str, ns: &str) -> bool {
    self.has_property_ns(name, ns)
  }

  /// Alias for has_property_no_ns
  pub fn has_attribute_no_ns(&self, name: &str) -> bool {
    self.has_property_no_ns(name)
  }

  /// Sets the value of property `name` to `value`
  ///
  /// The new value of an existing attribute replaces its value nodes, which libxml2 frees: their wrappers read as
  /// `Node::null()` afterwards.
  pub fn set_property(
    &mut self,
    name: &str,
    value: &str,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    if VALUE_NODE_WRAPPED.load(Ordering::Relaxed) || node_sets_live() {
      return self.set_property_neutralizing(name, value, None);
    }
    with_c_strs(name, value, |c_name, c_value| {
      let node_ptr = self.node_ptr_mut()?;
      unsafe { xmlSetProp(node_ptr, c_name, c_value) };
      Ok(())
    })?
  }
  /// Sets a namespaced attribute
  ///
  /// The new value of an existing attribute replaces its value nodes, which libxml2 frees: their wrappers read as
  /// `Node::null()` afterwards.
  pub fn set_property_ns(
    &mut self,
    name: &str,
    value: &str,
    ns: &Namespace,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    self.refuse_foreign_namespace(Some(ns))?;
    if VALUE_NODE_WRAPPED.load(Ordering::Relaxed) || node_sets_live() {
      return self.set_property_neutralizing(name, value, Some(ns));
    }
    with_c_strs(name, value, |c_name, c_value| {
      let node_ptr = self.node_ptr_mut()?;
      unsafe { xmlSetNsProp(node_ptr, ns.ns_ptr(), c_name, c_value) };
      Ok(())
    })?
  }

  /// `set_property` (no `ns`) or `set_property_ns`, once a value node may have been wrapped ([`VALUE_NODE_WRAPPED`]), or
  /// while an XPath node set is live, which may hold a value node (an evaluation at one selects it, or its siblings):
  /// also neutralizes the wrappers of the old value nodes libxml2 frees, and logs those frees. Out of line, so that the
  /// setters stay small.
  #[inline(never)]
  fn set_property_neutralizing(
    &mut self,
    name: &str,
    value: &str,
    ns: Option<&Namespace>,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    with_c_strs(name, value, |c_name, c_value| {
      let node_ptr = self.node_ptr_mut()?;
      let values = ValueNodes::collect(self, name, c_name, ns.is_none());
      match ns {
        None => unsafe { xmlSetProp(node_ptr, c_name, c_value) },
        Some(ns) => unsafe { xmlSetNsProp(node_ptr, ns.ns_ptr(), c_name, c_value) },
      };
      if let Some(values) = values {
        values.neutralize_replaced(self);
      }
      Ok(())
    })?
  }

  /// Neutralizes the wrappers of the attribute `attr` and its value nodes, which libxml2 is about to free, once any may
  /// be cached ([`ATTRIBUTE_WRAPPED`], [`VALUE_NODE_WRAPPED`]), and logs the frees while an XPath node set is live. A
  /// DTD's default value, which `xmlHasProp` may also return, is not this element's to free and keeps its wrapper.
  fn neutralize_attribute(&self, attr: xmlAttrPtr) {
    if !ATTRIBUTE_WRAPPED.load(Ordering::Relaxed)
      && !VALUE_NODE_WRAPPED.load(Ordering::Relaxed)
      && !node_sets_live()
    {
      return;
    }
    let attr = attr as xmlNodePtr;
    if xmlGetNodeType(attr) == xmlElementType_XML_ATTRIBUTE_NODE
      && let Some(doc) = self.upgrade_docref()
    {
      doc.borrow_mut().prepare_to_free(attr);
    }
  }

  /// Removes the property of given `name`
  ///
  /// libxml2 frees the attribute: its wrappers, from `get_property_node` and the like, read as `Node::null()` afterwards.
  pub fn remove_property(&mut self, name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let attr_node = with_c_str(name, |c_name| -> Result<_, Box<dyn Error + Send + Sync>> {
      Ok(unsafe { xmlHasProp(self.node_ptr_mut()?, c_name) })
    })??;
    unsafe {
      if !attr_node.is_null() {
        self.neutralize_attribute(attr_node);
        let remove_prop_status = xmlRemoveProp(attr_node);
        if remove_prop_status == 0 {
          Ok(())
        } else {
          // Propagate libxml2 failure to remove
          Err(From::from(format!(
            "libxml2 failed to remove property with status: {remove_prop_status:?}"
          )))
        }
      } else {
        // silently no-op if asked to remove a property which is not present
        Ok(())
      }
    }
  }

  /// Removes the property of given `name` and namespace (`ns`)
  ///
  /// libxml2 frees the attribute: its wrappers, from `get_property_node` and the like, read as `Node::null()` afterwards.
  pub fn remove_property_ns(
    &mut self,
    name: &str,
    ns: &str,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    let attr_node = with_c_strs(
      name,
      ns,
      |c_name, c_ns| -> Result<_, Box<dyn Error + Send + Sync>> {
        Ok(unsafe { xmlHasNsProp(self.node_ptr_mut()?, c_name, c_ns) })
      },
    )??;
    unsafe {
      if !attr_node.is_null() {
        self.neutralize_attribute(attr_node);
        let remove_prop_status = xmlRemoveProp(attr_node);
        if remove_prop_status == 0 {
          Ok(())
        } else {
          // Propagate libxml2 failure to remove
          Err(From::from(format!(
            "libxml2 failed to remove property with status: {remove_prop_status:?}"
          )))
        }
      } else {
        // silently no-op if asked to remove a property which is not present
        Ok(())
      }
    }
  }

  /// Removes the property of given `name` with no namespace
  ///
  /// libxml2 frees the attribute: its wrappers, from `get_property_node` and the like, read as `Node::null()` afterwards.
  pub fn remove_property_no_ns(&mut self, name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let attr_node = with_c_str(name, |c_name| -> Result<_, Box<dyn Error + Send + Sync>> {
      Ok(unsafe { xmlHasNsProp(self.node_ptr_mut()?, c_name, ptr::null()) })
    })??;
    if !attr_node.is_null() {
      self.neutralize_attribute(attr_node);
      let remove_prop_status = unsafe { xmlRemoveProp(attr_node) };
      if remove_prop_status == 0 {
        Ok(())
      } else {
        // Propagate libxml2 failure to remove
        Err(From::from(format!(
          "libxml2 failed to remove property with status: {remove_prop_status:?}"
        )))
      }
    } else {
      // silently no-op if asked to remove a property which is not present
      Ok(())
    }
  }

  /// Alias for get_property
  pub fn get_attribute(&self, name: &str) -> Option<String> {
    self.get_property(name)
  }

  /// Alias for with_property
  pub fn with_attribute<R>(&self, name: &CStr, f: impl FnOnce(&str) -> R) -> Option<R> {
    self.with_property(name, f)
  }

  /// Calls `f` with a copy of this node's name, as [`get_name`](Node::get_name) reads it (`""` if none), without
  /// allocating for a name of at most 128 bytes. As [`RoNode::with_name`]; `f` may change the document, this node
  /// included.
  pub fn with_name<R>(&self, f: impl FnOnce(&str) -> R) -> R {
    self.as_readonly().with_name(f)
  }

  /// Calls `f` with [`get_content`](Node::get_content)'s value, without allocating for the short content of a text,
  /// CDATA, comment or PI node. As [`RoNode::with_content`]; `f` may change the document, this node included.
  pub fn with_content<R>(&self, f: impl FnOnce(&str) -> R) -> R {
    self.as_readonly().with_content(f)
  }

  /// Calls `f` with the namespace prefix: `None` without a namespace, `Some("")` for a default namespace. As
  /// [`RoNode::with_namespace_prefix`]; `f` may change the document, this node included.
  pub fn with_namespace_prefix<R>(&self, f: impl FnOnce(Option<&str>) -> R) -> R {
    self.as_readonly().with_namespace_prefix(f)
  }

  /// Calls `f` with the namespace URI; `None` without a namespace. As [`RoNode::with_namespace_href`]; `f` may change
  /// the document, this node included.
  pub fn with_namespace_href<R>(&self, f: impl FnOnce(Option<&str>) -> R) -> R {
    self.as_readonly().with_namespace_href(f)
  }

  /// Calls `f(name, value)` for each property in document order, without allocating for a short value; nothing on a
  /// non-element. Unlike [`get_properties`](Node::get_properties), it reports every same-named property from different
  /// namespaces. As [`RoNode::for_each_property`], except that `f` may also free this node: the walk then stops.
  /// After each call, the element's property list is searched again (for the property `f` may have removed), so the
  /// cost grows with the square of the property count.
  pub fn for_each_property(&self, mut f: impl FnMut(&str, &str)) {
    walk_properties(|| self.node_ptr(), false, |name, _, value| f(name, value))
  }

  /// As [`for_each_property`](Node::for_each_property), also passing the namespace URI (`None` if none).
  pub fn for_each_property_ns(&self, f: impl FnMut(&str, Option<&str>, &str)) {
    walk_properties(|| self.node_ptr(), true, f)
  }

  /// Alias for [`Node::for_each_property`]
  pub fn for_each_attribute(&self, f: impl FnMut(&str, &str)) {
    self.for_each_property(f)
  }

  /// Alias for [`Node::for_each_property_ns`]
  pub fn for_each_attribute_ns(&self, f: impl FnMut(&str, Option<&str>, &str)) {
    self.for_each_property_ns(f)
  }

  /// Alias for get_property_ns
  pub fn get_attribute_ns(&self, name: &str, ns: &str) -> Option<String> {
    self.get_property_ns(name, ns)
  }

  /// Alias for get_property_no_ns
  pub fn get_attribute_no_ns(&self, name: &str) -> Option<String> {
    self.get_property_no_ns(name)
  }

  /// Alias for get_property_node
  pub fn get_attribute_node(&self, name: &str) -> Option<Node> {
    self.get_property_node(name)
  }

  /// Alias for get_property_node_ns
  pub fn get_attribute_node_ns(&self, name: &str, ns: &str) -> Option<Node> {
    self.get_property_node_ns(name, ns)
  }

  /// Alias for get_property_node_no_ns
  pub fn get_attribute_node_no_ns(&self, name: &str) -> Option<Node> {
    self.get_property_node_no_ns(name)
  }

  /// Alias for set_property
  pub fn set_attribute(
    &mut self,
    name: &str,
    value: &str,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
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
  pub fn remove_attribute_ns(
    &mut self,
    name: &str,
    ns: &str,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    self.remove_property_ns(name, ns)
  }

  /// Alias for remove_property_no_ns
  pub fn remove_attribute_no_ns(&mut self, name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    self.remove_property_no_ns(name)
  }

  /// Get a copy of the attributes of this node
  pub fn get_properties(&self) -> HashMap<String, String> {
    let first_prop = xmlGetFirstProperty(self.node_ptr());
    let mut count = 0;
    let mut current_prop = first_prop;
    while !current_prop.is_null() {
      count += 1;
      current_prop = xmlNextPropertySibling(current_prop);
    }
    let mut attributes = HashMap::with_capacity(count);

    let mut current_prop = first_prop;
    while !current_prop.is_null() {
      let name_ptr = xmlAttrName(current_prop);
      if name_ptr.is_null() {
        // A property with a NULL name can't be keyed; skip it rather than
        // `strlen(NULL)` inside `CStr::from_ptr`.
        current_prop = xmlNextPropertySibling(current_prop);
        continue;
      }
      let name = c_str_to_string(unsafe { CStr::from_ptr(name_ptr) });
      // Read the value straight from the attribute node we already hold, rather
      // than re-resolving it by name: `get_property` calls `xmlGetProp`, which
      // re-scans the whole attribute list with `xmlStrEqual` and allocates a
      // fresh `CString` for the name on every call — quadratic over the
      // attribute count, and a hot path during document build / math parsing.
      let value = attr_node_value(current_prop);
      attributes.insert(name, value);
      current_prop = xmlNextPropertySibling(current_prop);
    }

    attributes
  }

  /// Get a copy of this node's attributes and their namespaces
  pub fn get_properties_ns(&self) -> HashMap<(String, Option<Namespace>), String> {
    let mut attributes = HashMap::new();
    let node_ptr = self.node_ptr();
    // The node's document, for its namespaces: upgraded at the first one.
    let mut document = None;

    let mut current_prop = xmlGetFirstProperty(node_ptr);
    while !current_prop.is_null() {
      let name_ptr = xmlAttrName(current_prop);
      if name_ptr.is_null() {
        // A property with a NULL name can't be keyed; skip it rather than
        // `strlen(NULL)` inside `CStr::from_ptr`.
        current_prop = xmlNextPropertySibling(current_prop);
        continue;
      }
      let name = c_str_to_string(unsafe { CStr::from_ptr(name_ptr) });
      // Same direct-read optimization as `get_properties`: the value is read
      // from the attribute node itself, avoiding a by-name `xmlGetNsProp`
      // re-scan (and a per-attribute `CString` allocation).
      let value = attr_node_value(current_prop);
      let ns_ptr = xmlAttrNs(current_prop);
      if ns_ptr.is_null() {
        attributes.insert((name, None), value);
      } else {
        let (doc_ptr, document) =
          document.get_or_insert_with(|| (xmlGetDoc(node_ptr), self.upgrade_docref()));
        let ns = Namespace::of(ns_ptr, *doc_ptr, document.clone());
        attributes.insert((name, Some(ns)), value);
      }
      current_prop = xmlNextPropertySibling(current_prop);
    }

    attributes
  }

  /// Alias for `get_properties`
  pub fn get_attributes(&self) -> HashMap<String, String> {
    self.get_properties()
  }

  /// Alias for `get_properties_ns`
  pub fn get_attributes_ns(&self) -> HashMap<(String, Option<Namespace>), String> {
    self.get_properties_ns()
  }

  /// Gets the active namespace associated of this node
  pub fn get_namespace(&self) -> Option<Namespace> {
    let node = self.0.borrow();
    let ns_ptr = xmlNodeNs(node.node_ptr);
    if ns_ptr.is_null() {
      None
    } else {
      Some(Namespace::of(
        ns_ptr,
        xmlGetDoc(node.node_ptr),
        node.document.upgrade(),
      ))
    }
  }

  /// Gets a list of namespaces associated with this node.
  ///
  /// `xmlGetNsList` mallocs an ARRAY of `xmlNsPtr` that the CALLER must
  /// free — the namespaces it points at belong to the document and must NOT
  /// be freed. The historical TODO here tried `xmlFreeNs(list)` (frees the
  /// first namespace struct — hence the remembered segfault) and gave up,
  /// leaking the array on EVERY call: measured downstream at 2,429 lost
  /// blocks over 30 documents (valgrind), a leading term of a ~150 KB/page
  /// RSS climb across a 115,000-page render whose materialization path calls
  /// this per node. The array is freed with the crate's per-target
  /// allocator shim (`bindgenFree` — NOT the `xmlFree` global, which is not
  /// a linkable symbol on MSVC).
  pub fn get_namespaces(&self, doc: &Document) -> Vec<Namespace> {
    let node = self.0.borrow();
    let list_ptr_raw = unsafe { xmlGetNsList(doc.doc_ptr(), node.node_ptr) };
    if list_ptr_raw.is_null() {
      Vec::new()
    } else {
      let mut namespaces = Vec::new();
      // The node's own document, which may not be `doc`.
      let doc_ptr = xmlGetDoc(node.node_ptr);
      let document = node.document.upgrade();
      let mut ptr_iter = list_ptr_raw as *mut xmlNsPtr;
      unsafe {
        while !ptr_iter.is_null() && !(*ptr_iter).is_null() {
          namespaces.push(Namespace::of(*ptr_iter, doc_ptr, document.clone()));
          ptr_iter = ptr_iter.add(1);
        }
      }
      crate::c_helpers::bindgenFree(list_ptr_raw as *mut std::os::raw::c_void);
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
    let node = self.0.borrow();
    // The node's document: upgraded at the first declaration.
    let mut document = None;
    let mut ns_ptr = xmlNodeNsDeclarations(node.node_ptr);
    while !ns_ptr.is_null() {
      if !xmlNsPrefix(ns_ptr).is_null() || !xmlNsHref(ns_ptr).is_null() {
        let (doc_ptr, document) =
          document.get_or_insert_with(|| (xmlGetDoc(node.node_ptr), node.document.upgrade()));
        namespaces.push(Namespace::of(ns_ptr, *doc_ptr, document.clone()));
      }
      ns_ptr = xmlNextNsSibling(ns_ptr);
    }
    namespaces
  }

  /// Sets a `Namespace` for the node
  pub fn set_namespace(
    &mut self,
    namespace: &Namespace,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    let node_ptr = self.node_ptr_mut()?;
    if !namespace.may_be_used_in(xmlGetDoc(node_ptr)) {
      return Err(foreign_namespace());
    }
    unsafe {
      xmlSetNs(node_ptr, namespace.ns_ptr());
    }
    Ok(())
  }

  /// `Err` for a namespace that is not of this node's document, which the node would point into after it drops.
  #[inline]
  fn refuse_foreign_namespace(
    &self,
    namespace: Option<&Namespace>,
  ) -> Result<(), Box<dyn Error + Send + Sync>> {
    match namespace {
      Some(ns) if !ns.may_be_used_in(xmlGetDoc(self.node_ptr())) => Err(foreign_namespace()),
      _ => Ok(()),
    }
  }

  /// Looks up the prefix of a namespace from its URI, basedo around a given `Node`
  pub fn lookup_namespace_prefix(&self, href: &str) -> Option<String> {
    if href.is_empty() {
      return None;
    }
    let ptr_mut = self.node_ptr();
    let ns_ptr = with_c_str_or_else(
      href,
      |_| ptr::null_mut(),
      |c_href| unsafe { xmlSearchNsByHref(xmlGetDoc(ptr_mut), ptr_mut, c_href) },
    );
    if !ns_ptr.is_null() {
      Some(namespace::prefix_of(ns_ptr))
    } else {
      None
    }
  }

  /// Looks up the uri of a namespace from its prefix, basedo around a given `Node`
  pub fn lookup_namespace_uri(&self, prefix: &str) -> Option<String> {
    if prefix.is_empty() {
      return None;
    }
    let ns_ptr = with_c_str_or_else(
      prefix,
      |_| ptr::null_mut(),
      |c_prefix| unsafe { xmlSearchNs(xmlGetDoc(self.node_ptr()), self.node_ptr(), c_prefix) },
    );
    if !ns_ptr.is_null() {
      Some(namespace::href_of(ns_ptr)).filter(|href| !href.is_empty())
    } else {
      None
    }
  }

  /// Removes the namespaces of this `Node` and its descendants: their own, their attributes', and every namespace
  /// declaration among them. The document keeps the declarations until it is freed, so a `Namespace` handed out for
  /// one never dangles (#235). An entity reference's children belong to the entity's declaration and are left alone.
  pub fn recursively_remove_namespaces(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
    let top = self.node_ptr_mut()?;
    let doc = self.get_docref().upgrade();
    let mut doc = doc.as_ref().map(|doc| doc.borrow_mut());
    for_each_freed_with(top, |node_ptr| {
      // `xmlSetNs` sets the namespace of an element or an attribute, and ignores any other node.
      let node_type = xmlGetNodeType(node_ptr);
      if node_type == xmlElementType_XML_ELEMENT_NODE
        || node_type == xmlElementType_XML_ATTRIBUTE_NODE
      {
        unsafe { xmlSetNs(node_ptr, ptr::null_mut()) };
      }
      if let Some(doc) = doc.as_mut() {
        doc.keep_namespaces(node_ptr);
      }
    });
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
  ///
  /// libxml2 merges a text `child` into an adjacent text node and frees it, as it frees a `child` added to a text
  /// node, after appending its content. An attribute `child` replaces (frees) the attribute of the same name. A freed
  /// node's wrappers read as `Node::null()` afterwards: here, `child`'s, when it was merged.
  ///
  /// A linked `child` is moved: it is unlinked first, as libxml2 does since 2.13 (before, `xmlAddChild` left it in its
  /// old parent's children too), unless it is this node's last child already, which libxml2 leaves in place. An
  /// attribute `child` is added to an element only, as libxml2 required before 2.13; since, it adds one to any node,
  /// at a field that a document's or a DTD's own data overlays. An entity reference takes no child: its child is its
  /// entity's declaration, which libxml2 would link the new one after, among the DTD's children.
  pub fn add_child(&mut self, child: &mut Node) -> Result<(), String> {
    if let Some(refusal) = self.link_refusal(child) {
      return Err(refusal.to_string());
    }
    let parent_ptr = self.node_ptr_mut()?;
    let child_ptr = child.node_ptr_mut()?;
    if parent_ptr.is_null() {
      return Err("add_child encountered NULL pointer".to_string());
    }
    let parent_type = xmlGetNodeType(parent_ptr);
    if parent_type == xmlElementType_XML_ENTITY_REF_NODE {
      return Err(
        "an entity reference takes no child: its child is its entity's declaration".to_string(),
      );
    }
    if xmlGetNodeType(child_ptr) == xmlElementType_XML_ATTRIBUTE_NODE
      && parent_type != xmlElementType_XML_ELEMENT_NODE
    {
      return Err("an attribute can only be added to an element".to_string());
    }
    if !xmlGetParent(child_ptr).is_null() {
      // SAFETY: `parent_ptr` is a live node, not NULL.
      if unsafe { (*parent_ptr).last } == child_ptr {
        // Its last child already: libxml2 leaves it in place since 2.13, where moving it could merge it into a text
        // node before it.
        child.set_linked();
        return Ok(());
      }
      unsafe { xmlUnlinkNode(child_ptr) };
    }
    let casualties = InsertionCasualties::collect(child_ptr, parent_ptr, true);
    let new_child_ptr = unsafe { xmlAddChild(parent_ptr, child_ptr) };
    if let Some(casualties) = casualties {
      casualties.neutralize_freed(new_child_ptr, [&*self, &*child]);
    }
    child.settle_linkage(new_child_ptr);
    if new_child_ptr.is_null() {
      Err("add_child encountered NULL pointer".to_string())
    } else {
      Ok(())
    }
  }

  /// Creates a new `Node` as child to the self `Node`
  ///
  /// Returns `Err` if this node is not an element, a document or a fragment, which libxml2 makes no child under.
  pub fn new_child(
    &mut self,
    ns: Option<Namespace>,
    name: &str,
  ) -> Result<Node, Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name)?;
    self.refuse_foreign_namespace(ns.as_ref())?;
    // (`ns` lives until the end, and with it its document.)
    let ns_ptr = ns.as_ref().map_or(ptr::null_mut(), Namespace::ns_ptr);
    unsafe {
      let new_ptr = xmlNewChild(
        self.node_ptr_mut()?,
        ns_ptr,
        c_name.as_bytes().as_ptr(),
        ptr::null(),
      );
      if new_ptr.is_null() {
        return Err(From::from(
          "xmlNewChild made no node: only an element, a document or a fragment takes a child",
        ));
      }
      Ok(Node::wrap(new_ptr, &self.get_docref().upgrade().unwrap()))
    }
  }

  /// Adds a new text child, to this `Node`
  ///
  /// Returns `Err` if this node is not an element, a document or a fragment, which libxml2 makes no child under.
  pub fn add_text_child(
    &mut self,
    ns: Option<Namespace>,
    name: &str,
    content: &str,
  ) -> Result<Node, Box<dyn Error + Send + Sync>> {
    let c_name = CString::new(name)?;
    let c_content = CString::new(content)?;
    self.refuse_foreign_namespace(ns.as_ref())?;
    // (`ns` lives until the end, and with it its document.)
    let ns_ptr = ns.as_ref().map_or(ptr::null_mut(), Namespace::ns_ptr);
    unsafe {
      let new_ptr = xmlNewTextChild(
        self.node_ptr_mut()?,
        ns_ptr,
        c_name.as_bytes().as_ptr(),
        c_content.as_bytes().as_ptr(),
      );
      if new_ptr.is_null() {
        return Err(From::from(
          "xmlNewTextChild made no node: only an element, a document or a fragment takes a child",
        ));
      }
      Ok(Node::wrap(new_ptr, &self.get_docref().upgrade().unwrap()))
    }
  }

  /// Append text to this `Node`
  ///
  /// Returns an error for text with a NUL byte, and for text of more than `i32::MAX` bytes, the most libxml2 appends
  /// at a time.
  ///
  /// An entity reference is left alone, as libxml2 2.13 and later do: its content is its entity's declaration's, which
  /// libxml2 before 2.13 appended to (#235).
  pub fn append_text(&mut self, content: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let c_len = c_int::try_from(content.len())
      .map_err(|_| "append_text: libxml2 appends at most i32::MAX bytes at a time")?;
    if c_len > 0 {
      let node_ptr = self.node_ptr_mut()?;
      if xmlGetNodeType(node_ptr) == xmlElementType_XML_ENTITY_REF_NODE {
        return Ok(());
      }
      let c_content = CString::new(content)?;
      unsafe {
        xmlNodeAddContentLen(node_ptr, c_content.as_bytes().as_ptr(), c_len);
      }
    }
    Ok(())
  }

  /// Unbinds the Node from its siblings and Parent, but not from the Document it belongs to.
  ///
  /// At the libxml2 level, `xmlUnlinkNode` severs `parent`/`prev`/`next`
  /// links but **leaves `node->doc` set** — the source document is
  /// still recorded as the lifetime owner of the underlying allocation.
  /// Consequences for subsequent handling:
  ///
  /// * **Re-attach** the node into a tree (via `add_child`,
  ///   `add_prev_sibling`, `add_next_sibling`, or
  ///   `Document::set_root_element`). Lifetime stays with the source
  ///   document; nothing more to do.
  /// * **Copy it out** into a new document via
  ///   [`Document::dup_node_into_new_doc`] (or a comparable libxml2
  ///   path). The copy is independent; the original is still detached.
  /// * **Discard the original**: call [`Node::set_rust_owned`] on it.
  ///   The wrapper's `Drop` will then `xmlFreeNode` the C subtree when
  ///   the last clone drops. Without this call, the detached subtree
  ///   leaks: the wrapper's `Drop` is a no-op (it must be, to avoid
  ///   freeing memory the source document still considers live), and
  ///   the source document's eventual `xmlFreeDoc` walks the tree
  ///   topology, so it never reaches a parent==NULL subtree.
  ///
  /// In short: an unlinked node that is *neither* re-attached *nor*
  /// marked `set_rust_owned` is a leak until process exit.
  ///
  /// A document's own node (an HTML one too) is no node of a tree: it is left alone.
  /// A DTD is the exception: its document's entity references point into it,
  /// so the document keeps it, frees it with itself, and refuses to link it
  /// again ([`Document::remove_internal_subset`] unlinks it the same way). A
  /// declaration in a DTD stays there: its DTD frees it.
  pub fn unlink_node(&mut self) {
    let node_type = self.get_type();
    if node_type.is_some()
      && !is_document(&node_type)
      && !is_declaration(&node_type)
      && !self.is_unlinked()
    {
      // only unlink nodes that are currently marked as linked
      self.set_unlinked();
      unsafe {
        xmlUnlinkNode(self.node_ptr());
      }
      if node_type == Some(NodeType::DTDNode) {
        self.keep_dtd();
      }
    }
  }
  /// Leaves this DTD, unlinked, to its document, which frees it (#235): see `free_node`.
  #[inline(never)]
  fn keep_dtd(&self) {
    if let Some(doc) = self.get_docref().upgrade()
      && let Ok(mut doc) = doc.try_borrow_mut()
    {
      doc.keep_dtd(self.node_ptr() as xmlDtdPtr);
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
  ///
  /// Returns `true` for both `Unlinked` and `RustOwned` states — both
  /// are detached from any document tree.
  pub fn is_unlinked(&self) -> bool {
    !matches!(self.0.borrow().linkage, Linkage::Linked)
  }

  /// Whether a wrapper other than this one shares this node's state (another clone of it is alive).
  pub(crate) fn is_shared(&self) -> bool {
    Rc::strong_count(&self.0) > 1
  }

  /// Nulls the pointer every clone of this wrapper shares, once libxml2 frees (or is about to free) its node: they all
  /// read as `Node::null()` from then on, and `Drop` frees nothing.
  pub(crate) fn null_out(&self) {
    if let Ok(mut inner) = self.0.try_borrow_mut() {
      inner.node_ptr = ptr::null_mut();
    }
  }

  /// This wrapper, held weakly.
  pub(crate) fn downgrade(&self) -> NodeWeak {
    Rc::downgrade(&self.0)
  }

  /// The wrapper `weak` holds, if a clone of it is still alive.
  pub(crate) fn upgrade(weak: &NodeWeak) -> Option<Node> {
    weak.upgrade().map(Node)
  }

  /// Releases this wrapper as the document at `doc_ptr` is freed, which frees the nodes it owns and their names:
  /// [`null_out`](Self::null_out), unless its node is an unlinked one the document does not own (whose `doc` is NULL
  /// or another document's). A rust-owned node is returned instead, for the document to free while it is alive.
  pub(crate) fn release(&self, doc_ptr: xmlDocPtr) -> Option<xmlNodePtr> {
    let Ok(mut inner) = self.0.try_borrow_mut() else {
      return None;
    };
    let node_ptr = inner.node_ptr;
    if node_ptr.is_null()
      || matches!(inner.linkage, Linkage::Unlinked) && xmlGetDoc(node_ptr) != doc_ptr
    {
      return None;
    }
    inner.node_ptr = ptr::null_mut();
    matches!(inner.linkage, Linkage::RustOwned).then_some(node_ptr)
  }

  /// Why `new` may not be linked next to or under this node, if it may not: a rust-owned node would be freed by its
  /// last wrapper's drop while linked, and a node of another document would be freed by that document while its
  /// wrapper, and before libxml2 2.12 its names, stayed with this one ([`Document::import_node`] copies it instead).
  /// No [`type_refusal`] type is linked (a document, or a DTD or a declaration in one), and no node under itself or
  /// its attributes, which libxml2 would make a loop of.
  #[inline(always)]
  pub(crate) fn link_refusal(&self, new: &Node) -> Option<&'static str> {
    let this = self.0.borrow();
    let new = new.0.borrow();
    let new_type = xmlGetNodeType(new.node_ptr);
    if matches!(new.linkage, Linkage::RustOwned) {
      Some("a rust-owned node cannot be linked: dropping its last wrapper frees it")
    } else if !Weak::ptr_eq(&this.document, &new.document) {
      Some("a node of another document cannot be linked; import_node copies it")
    } else if let Some(refusal) = type_refusal(new_type) {
      Some(refusal)
    } else if new.node_ptr == this.node_ptr && !new.node_ptr.is_null()
      || has_descendants(new.node_ptr, new_type) && is_within(this.node_ptr, new.node_ptr)
    {
      Some("a node cannot be linked under or next to itself")
    } else {
      None
    }
  }

  /// Marks this node, which an insertion was to link, linked if it did (`inserted` is not NULL), and unlinked if it
  /// did not and the node is detached: libxml2 unlinks some nodes it then refuses to insert.
  #[inline]
  fn settle_linkage(&self, inserted: xmlNodePtr) {
    if !inserted.is_null() {
      self.set_linked();
    } else {
      self.settle_refused();
    }
  }

  /// Links this node as the root element of the document at `doc_ptr` ([`Document::set_root_element`]). In a document
  /// with no element, libxml2 appends it to the document's children instead, and merges a text node into a text node
  /// there and frees it, as [`add_next_sibling`](Self::add_next_sibling) does: its wrappers read as `Node::null()`
  /// afterwards.
  ///
  /// Returns the root element this node replaced, which libxml2 unlinked, if any.
  pub(crate) fn link_as_root(&self, doc_ptr: xmlDocPtr) -> Option<xmlNodePtr> {
    let node_ptr = self.node_ptr();
    let casualties = InsertionCasualties::collect(node_ptr, doc_ptr as xmlNodePtr, false);
    let old_root = unsafe { xmlDocSetRootElement(doc_ptr, node_ptr) };
    let mut inserted = node_ptr;
    if let Some(casualties) = casualties {
      // `xmlDocSetRootElement` returns the element it replaced, not the node it inserted: this one, unless it merged
      // it into the node it would have followed, the document's last child now. Only addresses are compared: nothing
      // was allocated since the free.
      let mut child = xmlGetFirstChild(doc_ptr as xmlNodePtr);
      while !child.is_null() && child != node_ptr {
        child = xmlNextSibling(child);
      }
      if child.is_null() {
        // SAFETY: `doc_ptr` is a live document.
        inserted = unsafe { (*doc_ptr).last };
      }
      casualties.neutralize_freed(inserted, [self, self]);
    }
    self.settle_linkage(inserted);
    // `xmlDocSetRootElement` returns the root it found, also when that is this node, which it then left in place.
    (!old_root.is_null() && xmlGetParent(old_root).is_null()).then_some(old_root)
  }

  /// [`settle_linkage`](Self::settle_linkage), when libxml2 did not insert this node.
  #[cold]
  #[inline(never)]
  fn settle_refused(&self) {
    if xmlGetParent(self.node_ptr()).is_null() {
      self.set_unlinked();
    }
  }
  /// Take ownership of this detached node's C allocation.
  ///
  /// After this call, the C node will be freed via `xmlFreeNode` when the
  /// last `Node` clone drops, regardless of whether `node->doc` is still
  /// set. The flag is sticky and propagates to every clone (they share
  /// the same `_Node`), so transferring ownership once is sufficient.
  ///
  /// The free neutralizes every cached wrapper into the subtree first, as
  /// [`free_subtree`](Self::free_subtree) does: a descendant handed out
  /// earlier reads as `Node::null()` afterwards, not as freed memory.
  ///
  /// Use this on a subtree that has been:
  ///   1. detached from its parent via `unlink_node`, AND
  ///   2. either copied out into another document (e.g. via
  ///      `Document::dup_node_into_new_doc`) or otherwise will not be
  ///      re-attached to any tree.
  ///
  /// Without this call, detached nodes are *not* freed by the wrapper —
  /// they remain attributed to `node->doc` for safety, and `xmlFreeDoc`
  /// reclaims them only if they're still reachable from the doc root.
  /// A subtree that has been unlinked but not re-attached and not marked
  /// rust-owned will leak until process exit.
  ///
  /// # Panics
  ///
  /// On a node that is still `Linked` (part of a document tree): its
  /// later free would leave dangling pointers in its former parent,
  /// siblings and ID table. (Before 0.3.25, only debug builds checked.)
  pub fn set_rust_owned(&self) {
    let (node_ptr, doc) = {
      let mut inner = self.0.borrow_mut();
      assert!(
        !matches!(inner.linkage, Linkage::Linked),
        "set_rust_owned called on a Linked node — call unlink_node first"
      );
      if matches!(inner.linkage, Linkage::RustOwned) {
        return;
      }
      inner.linkage = Linkage::RustOwned;
      if !inner.unlinked_from_tree {
        return;
      }
      (inner.node_ptr, inner.document.upgrade())
    };
    // The document holds the wrapper weakly from now on, so that the last clone's drop frees the node.
    if let Some(doc) = doc
      && let Ok(mut doc) = doc.try_borrow_mut()
    {
      doc.own_node(node_ptr, self);
    }
  }

  /// Returns whether the C allocation is owned by the Rust wrapper.
  /// See `set_rust_owned`.
  pub fn is_rust_owned(&self) -> bool {
    matches!(self.0.borrow().linkage, Linkage::RustOwned)
  }

  /// This node as a `RoNode`. Adds nothing to the wrapper cache.
  #[must_use]
  pub fn as_readonly(&self) -> RoNode {
    RoNode(self.node_ptr())
  }

  /// [`Document::node_from_readonly`] on this node's document.
  ///
  /// # Panics
  /// If this node is null or its document was dropped.
  #[must_use]
  pub fn node_from_readonly(&self, node: RoNode) -> Node {
    let doc_ref = self
      .0
      .borrow()
      .document
      .upgrade()
      .expect("node_from_readonly: the document was dropped");
    Document(doc_ref).node_from_readonly(node)
  }

  /// Detach this node and free its entire C subtree IMMEDIATELY.
  ///
  /// This is the discard operation `unlink_node` alone does not provide: an
  /// unlinked doc-owned node is freed by nobody (see `unlink_node`), so a
  /// caller discarding a subtree for good must transfer ownership somewhere.
  /// `set_rust_owned` defers the free to the last wrapper drop — but a stray
  /// clone parked in a long-lived collection postpones that free past the
  /// owning document's `xmlFreeDoc`, and the eventual `xmlFreeNode` then
  /// reads the freed document (its dictionary) — a use-after-free.
  ///
  /// `free_subtree` instead frees NOW, after *neutralizing* every registered
  /// Rust wrapper into the subtree: each wrapper's shared inner `node_ptr`
  /// is nulled — clones held anywhere become inert (`Drop` is a no-op, the
  /// null-guarded accessors return their empty defaults) — and the
  /// bookkeeping entries are cleared, so a future allocation reusing one of
  /// the freed addresses cannot collide with a stale wrapper. Attribute
  /// nodes are neutralized along with elements and texts; attribute values
  /// are owned by their nodes and freed by `xmlFreeNode` itself, while the
  /// document keeps the namespace declarations, which a `Namespace` may
  /// point at, until it is freed. An entity reference's children belong to
  /// the entity's declaration, which `xmlFreeNode` leaves alone, and so does
  /// the neutralizing. (Since 0.3.25, dropping the last wrapper of a
  /// rust-owned node neutralizes the same way before its `xmlFreeNode`.)
  ///
  /// Documents (and document-fragment shells) are refused — tearing a whole
  /// document down belongs to `Document`'s `Drop`. A DTD is only unlinked:
  /// its document's entity references point into it, so the document keeps
  /// it, and frees it with itself. A declaration in a DTD (of an element, an
  /// attribute or an entity) is left alone: its DTD frees it.
  ///
  /// # Safety contract (mirrors C `xmlFreeNode`)
  ///
  /// The subtree must be garbage to the caller. Every `Node` into it is
  /// neutralized, but a `RoNode` into it is a bare pointer, and dangles.
  pub fn free_subtree(mut self) {
    let node_type = self.get_type();
    if is_document(&node_type) || is_declaration(&node_type) {
      return;
    }
    // Sever from parent/siblings.
    self.unlink_node();
    let top = self.0.borrow().node_ptr;
    if top.is_null() {
      return; // already freed/neutralized through another handle
    }
    if xmlGetNodeType(top) == xmlElementType_XML_DTD_NODE {
      return; // `unlink_node` left it to its document, which frees it.
    }
    if let Some(doc) = self.get_docref().upgrade() {
      doc.borrow_mut().prepare_to_free(top);
    }
    // Neutralize this handle itself, which the walk above could not reach
    // if the document is gone, then free.
    self.0.borrow_mut().node_ptr = ptr::null_mut();
    unsafe { xmlFreeNode(top) };
  }

  fn ptr_as_option(&self, node_ptr: xmlNodePtr) -> Option<Node> {
    if node_ptr.is_null() {
      None
    } else {
      // (the document upgraded in place, not through a cloned `Weak`)
      let doc_ref = self.0.borrow().document.upgrade().unwrap();
      let new_node = Node::wrap(node_ptr, &doc_ref);
      Some(new_node)
    }
  }

  /// internal helper to ensure the node is marked as linked/imported/adopted in the main document tree.
  ///
  /// Transitions from `Unlinked` to `Linked`. If the node is already
  /// `RustOwned`, this is a no-op in release builds and a debug-assert
  /// failure in debug builds: the caller has explicitly taken ownership
  /// and re-linking it would set up a guaranteed double-free
  /// (the new parent's `xmlFreeDoc` will free the subtree, then this
  /// wrapper's `Drop` will free it again). There is no public path to
  /// untake ownership; the right fix is for the caller to never re-link
  /// a `RustOwned` node.
  ///
  /// Public since 0.3.20: a consumer holding long-lived `Node` handles into a
  /// document (id caches and the like) needs to declare "the document owns
  /// this memory" before dropping the handles — some of which may have been
  /// `unlink_node`ed by intermediate processing. Without this, the only safe
  /// teardown was to leak each wrapper's allocation (latexml-oxide's
  /// `DocOwnedNode`, ~100+ bytes per id-cache entry per document — measured
  /// as a leading retention term at ~150 KB/page across a 115k-page render).
  #[inline]
  pub fn set_linked(&self) {
    let mut inner = self.0.borrow_mut();
    debug_assert!(
      !matches!(inner.linkage, Linkage::RustOwned),
      "set_linked called on a RustOwned node — re-linking would set up a double free; \
       drop the wrapper before re-attaching, or do not call set_rust_owned"
    );
    if matches!(inner.linkage, Linkage::Unlinked) {
      inner.linkage = Linkage::Linked;
      inner.unlinked_from_tree = false;
    }
  }

  /// internal helper to ensure the node is marked as unlinked/removed from the main document tree.
  ///
  /// Transitions from `Linked` to `Unlinked`. Leaves `RustOwned`
  /// untouched — that variant already implies "detached".
  ///
  /// The document keeps the wrapper cached, as a linked node's: wrapping the node again, while it is unlinked or
  /// after it is linked anywhere, finds the same wrapper, and the neutralizing frees and the document's drop reach it.
  pub(crate) fn set_unlinked(&self) {
    let mut inner = self.0.borrow_mut();
    if matches!(inner.linkage, Linkage::Linked) {
      inner.linkage = Linkage::Unlinked;
      inner.unlinked_from_tree = true;
    }
  }

  /// find nodes via xpath, at a specified node or the document root
  pub fn findnodes(&self, xpath: &str) -> Result<Vec<Node>, ()> {
    let mut context = Context::from_node(self)?;
    context.findnodes(xpath, Some(self))
  }

  /// Search this node for XPath `path`, and return only the first match.
  pub fn at_xpath(&self, path: &str, ns_binlings: &[(&str, &str)]) -> Result<Option<Node>, ()> {
    let mut context = Context::from_node(self)?;
    for (prefix, href) in ns_binlings {
      context.register_namespace(prefix, href)?;
    }
    let nodes = context.findnodes(path, Some(self))?;

    Ok(nodes.first().cloned())
  }

  /// Get a list of ancestor Node for this Node.
  pub fn ancestors(&self) -> Vec<Node> {
    let node_ptr = self.node_ptr();

    let mut res = Vec::new();

    let ancestor_ptrs = node_ancestors(node_ptr);

    for ptr in ancestor_ptrs {
      if let Some(node) = self.ptr_as_option(ptr) {
        res.push(node)
      }
    }

    res
  }

  /// find String values via xpath, at a specified node or the document root
  pub fn findvalues(&self, xpath: &str) -> Result<Vec<String>, ()> {
    let mut context = Context::from_node(self)?;
    context.findvalues(xpath, Some(self))
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
      match old.get_parent() {
        Some(old_parent) => {
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
        }
        _ => Err(From::from(format!(
          "Old node was not a child of {:?} parent. No registered parent exists.",
          self.get_name()
        ))),
      }
    } else {
      Err(From::from(
        "Can only call replace_child_node an a NodeType::Element type parent.",
      ))
    }
  }
}

fn node_ancestors(node_ptr: xmlNodePtr) -> Vec<xmlNodePtr> {
  if node_ptr.is_null() {
    return Vec::new();
  }

  let mut parent_ptr = xmlGetParent(node_ptr);

  if parent_ptr.is_null() {
    Vec::new()
  } else {
    let mut parents = vec![parent_ptr];

    while !xmlGetParent(parent_ptr).is_null() {
      parent_ptr = xmlGetParent(parent_ptr);
      parents.push(parent_ptr);
    }

    parents
  }
}

/// Read an attribute node's value directly via `xmlNodeGetContent`, freeing the
/// libxml2-allocated buffer. Equivalent to resolving the attribute by name with
/// `xmlGetProp`, but without the list re-scan and name `CString` allocation —
/// the caller already holds the attribute pointer.
pub(crate) fn attr_node_value(attr: xmlAttrPtr) -> String {
  if let Some(content) = unsafe { single_text_value(attr) } {
    return c_str_to_string(content);
  }
  let content_ptr = unsafe { xmlNodeGetContent(attr as xmlNodePtr) };
  if content_ptr.is_null() {
    return String::new();
  }
  let c_string = unsafe { CStr::from_ptr(content_ptr as *const c_char) };
  let value = c_str_to_string(c_string);
  bindgenFree(content_ptr as *mut c_void);
  value
}

/// The value of an attribute that libxml2 holds with no children (`""`) or as a single text or CDATA node (its
/// content): the shapes libxml2's own `xmlGetPropNodeValueInternal` (behind `xmlGetProp`, `xmlGetNsProp`,
/// `xmlGetNoNsProp`) reads without walking the children. `None` for any other shape (an entity reference in the value,
/// a DTD default declaration), which the caller reads through libxml2.
///
/// # Safety
/// `attr` is null or points to a live attribute (or attribute declaration); the borrow lasts while it is not changed.
pub(crate) unsafe fn single_text_value<'a>(attr: xmlAttrPtr) -> Option<&'a CStr> {
  if attr.is_null() || unsafe { (*attr).type_ } != xmlElementType_XML_ATTRIBUTE_NODE {
    return None;
  }
  let text = unsafe { (*attr).children };
  if text.is_null() {
    return Some(c"");
  }
  let text = unsafe { &*text };
  if text.next.is_null()
    && (text.type_ == xmlElementType_XML_TEXT_NODE
      || text.type_ == xmlElementType_XML_CDATA_SECTION_NODE)
    && !text.content.is_null()
  {
    Some(unsafe { CStr::from_ptr(text.content as *const c_char) })
  } else {
    None
  }
}

/// The value of `attr`, which libxml2's lookup (`xmlHasProp`, `xmlHasNsProp`) returned: `None` when it is NULL, read in
/// place when it is one of the shapes `single_text_value` reads, and otherwise copied by `getter`, the libxml2 getter
/// that makes the same lookup. libxml2's getters return NULL whenever that lookup does, so an absent property is
/// looked up once, not again by `getter`.
///
/// # Safety
/// As for `single_text_value`; `getter` makes the lookup that returned `attr`.
unsafe fn property_value(
  attr: xmlAttrPtr,
  getter: impl FnOnce() -> *mut xmlChar,
) -> Option<String> {
  if attr.is_null() {
    return None;
  }
  match unsafe { single_text_value(attr) } {
    Some(content) => Some(c_str_to_string(content)),
    None => take_xml_string(getter()),
  }
}

/// The string a libxml2 getter returned, as a `String`, freeing libxml2's copy; `None` when the getter returned NULL.
fn take_xml_string(value_ptr: *mut xmlChar) -> Option<String> {
  if value_ptr.is_null() {
    return None;
  }
  let value = c_str_to_string(unsafe { CStr::from_ptr(value_ptr as *const c_char) });
  bindgenFree(value_ptr as *mut c_void);
  Some(value)
}

/// A copy of `c` as a `String`, as `c.to_string_lossy().into_owned()` gives it; valid UTF-8 (almost every libxml2
/// string: a document parsed with its encoding ignored, or recovered, may hold other bytes) is checked by
/// `str::from_utf8`'s fast path instead of the lossy decoder's.
fn c_str_to_string(c: &CStr) -> String {
  match c.to_str() {
    Ok(s) => s.to_owned(),
    Err(_) => c.to_string_lossy().into_owned(),
  }
}

/// [`Node::with_property`] on `node_ptr` (NULL or a live node): `f` gets the value as [`with_str_copy`] copies it.
pub(crate) fn with_property_copy<R>(
  node_ptr: xmlNodePtr,
  name: &CStr,
  f: impl FnOnce(&str) -> R,
) -> Option<R> {
  if node_ptr.is_null() {
    return None;
  }
  let c_name = name.as_ptr() as *const xmlChar;
  // The value is copied out of libxml2's memory before `f` runs: a change `f` makes may free that memory.
  unsafe {
    let attr = xmlHasProp(node_ptr, c_name);
    if attr.is_null() {
      return None;
    }
    match single_text_value(attr) {
      Some(content) => Some(with_str_copy(content, f)),
      None => Some(f(&take_xml_string(xmlGetProp(node_ptr, c_name))?)),
    }
  }
}

/// Pass `f` a copy of `c`, decoded lossily if it is not UTF-8: on the stack when it fits in 128 bytes, otherwise in a
/// `String`. `c` is not read once `f` runs, so `f` may free it.
pub(crate) fn with_str_copy<R>(c: &CStr, f: impl FnOnce(&str) -> R) -> R {
  let bytes = c.to_bytes();
  let mut buffer = [0u8; 128];
  match buffer.get_mut(..bytes.len()) {
    Some(copy) => {
      copy.copy_from_slice(bytes);
      match str::from_utf8(copy) {
        Ok(s) => f(s),
        Err(_) => f(&String::from_utf8_lossy(copy)),
      }
    }
    None => f(&c_str_to_string(c)),
  }
}

/// The error for a namespace that is not of the node's document, which would point into its own document after that
/// drops.
#[cold]
fn foreign_namespace() -> Box<dyn Error + Send + Sync> {
  From::from(
    "the namespace is not of this node's document: declare it in that document, with Namespace::new",
  )
}

/// Pass `s` to `f` as a NUL-terminated string for libxml2: copied to the stack when it is shorter than 128 bytes,
/// otherwise through a `CString`. A string holding a NUL byte is no C string, and no name or value libxml2 holds: `nul`
/// makes the result instead of `f`. The stack path already checks for the byte, and returns `f`'s value as it is.
#[inline]
pub(crate) fn with_c_str_or_else<R>(
  s: &str,
  nul: impl FnOnce(NulError) -> R,
  f: impl FnOnce(*const xmlChar) -> R,
) -> R {
  const STACK: usize = 128;
  let bytes = s.as_bytes();
  if bytes.len() < STACK && !bytes.contains(&0) {
    let mut buffer = [0u8; STACK];
    buffer[..bytes.len()].copy_from_slice(bytes);
    f(buffer.as_ptr())
  } else {
    with_c_string(s, nul, f)
  }
}

/// [`with_c_str_or_else`]'s path for a long string or one holding a NUL byte, out of line.
#[cold]
#[inline(never)]
fn with_c_string<R>(
  s: &str,
  nul: impl FnOnce(NulError) -> R,
  f: impl FnOnce(*const xmlChar) -> R,
) -> R {
  match CString::new(s) {
    Ok(c_string) => f(c_string.as_bytes().as_ptr()),
    Err(error) => nul(error),
  }
}

/// [`with_c_str_or_else`] for two strings.
#[inline]
pub(crate) fn with_c_strs_or_else<R>(
  a: &str,
  b: &str,
  nul: impl Fn(NulError) -> R,
  f: impl FnOnce(*const xmlChar, *const xmlChar) -> R,
) -> R {
  with_c_str_or_else(a, &nul, |a| with_c_str_or_else(b, &nul, |b| f(a, b)))
}

/// [`with_c_str_or_else`], with the `NulError` as `Err`.
#[inline]
pub(crate) fn with_c_str<R>(s: &str, f: impl FnOnce(*const xmlChar) -> R) -> Result<R, NulError> {
  with_c_str_or_else(s, Err, |c| Ok(f(c)))
}

/// [`with_c_str`] for two strings.
#[inline]
pub(crate) fn with_c_strs<R>(
  a: &str,
  b: &str,
  f: impl FnOnce(*const xmlChar, *const xmlChar) -> R,
) -> Result<R, NulError> {
  with_c_strs_or_else(a, b, Err, |a, b| Ok(f(a, b)))
}

mod c14n;
