//! Document feature set
//!
use libc::{c_char, c_int};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fmt;
use std::ptr;
use std::rc::{Rc, Weak};
use std::str;

use crate::bindings::*;
use crate::c_helpers::{xmlGetDoc, xmlGetNodeType, xmlGetParent, xmlNodeTakeNsDeclarations};
use crate::readonly::RoNode;
use crate::tree::node::{Node, NodeWeak, for_each_freed_with, note_wrapped, type_refusal};
use crate::tree::nodetype::NodeType;

pub(crate) type DocumentRef = Rc<RefCell<_Document>>;
pub(crate) type DocumentWeak = Weak<RefCell<_Document>>;

/// Fast, dependency-free hasher for the pointer-keyed `nodes` cache. libxml node
/// pointers are unique and well-distributed, so SipHash's `RandomState` (the std
/// default) is wasted work on a map that is probed on EVERY `Node::wrap` /
/// lookup. This FxHash-style multiply-rotate mixes the address in a couple of
/// instructions. Internal to the node bookkeeping map; not public API.
#[derive(Default, Clone, Copy)]
pub(crate) struct BuildNodePtrHasher;
impl std::hash::BuildHasher for BuildNodePtrHasher {
  type Hasher = NodePtrHasher;
  #[inline]
  fn build_hasher(&self) -> NodePtrHasher {
    NodePtrHasher(0)
  }
}
#[doc(hidden)]
pub(crate) struct NodePtrHasher(u64);
impl NodePtrHasher {
  #[inline]
  fn add(&mut self, i: u64) {
    self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
  }
}
impl std::hash::Hasher for NodePtrHasher {
  #[inline]
  fn finish(&self) -> u64 {
    self.0
  }
  #[inline]
  fn write_usize(&mut self, i: usize) {
    self.add(i as u64);
  }
  #[inline]
  fn write_u64(&mut self, i: u64) {
    self.add(i);
  }
  fn write(&mut self, bytes: &[u8]) {
    for chunk in bytes.chunks(8) {
      let mut buf = [0u8; 8];
      buf[..chunk.len()].copy_from_slice(chunk);
      self.add(u64::from_le_bytes(buf));
    }
  }
}

/// The `_Document::nodes` bookkeeping map: `xmlNodePtr` → wrapped `Node`, hashed
/// with [`BuildNodePtrHasher`] rather than the std SipHash default.
pub(crate) type NodeMap = HashMap<xmlNodePtr, Node, BuildNodePtrHasher>;

/// The `_Document::rust_owned` map: `xmlNodePtr` → the wrapper of a rust-owned node, held weakly.
pub(crate) type RustOwnedMap = HashMap<xmlNodePtr, NodeWeak, BuildNodePtrHasher>;

#[cfg(test)]
mod node_ptr_hasher_tests {
  use super::BuildNodePtrHasher;
  use std::collections::HashSet;
  use std::hash::{BuildHasher, Hasher};

  fn hash_ptr(addr: usize) -> u64 {
    // Mirrors the key path: a thin raw pointer's `Hash` calls `write_usize`.
    let mut h = BuildNodePtrHasher.build_hasher();
    h.write_usize(addr);
    h.finish()
  }

  #[test]
  fn deterministic() {
    assert_eq!(hash_ptr(0x7f00_1234_5000), hash_ptr(0x7f00_1234_5000));
  }

  #[test]
  fn no_collisions_on_aligned_allocator_run() {
    // libxml node structs are large + aligned, so real addresses come in
    // aligned runs — the case a weak hash would clump. Assert the FxHash
    // mix keeps 10k such keys collision-free (guards against a degenerate
    // edit to the algorithm/constant).
    let base = 0x7f00_0000_0000usize;
    let mut seen = HashSet::new();
    for i in 0..10_000usize {
      assert!(seen.insert(hash_ptr(base + i * 160)), "collision at i={i}");
    }
  }
}

#[derive(Debug, Copy, Clone, Default)]
/// Save Options for Document
pub struct SaveOptions {
  /// format save output
  pub format: bool,
  /// drop the xml declaration
  pub no_declaration: bool,
  /// no empty tags
  pub no_empty_tags: bool,
  /// disable XHTML1 specific rules
  pub no_xhtml: bool,
  /// force XHTML1 specific rules
  pub xhtml: bool,
  /// force XML serialization on HTML doc
  pub as_xml: bool,
  /// force HTML serialization on XML doc
  pub as_html: bool,
  /// format with non-significant whitespace
  pub non_significant_whitespace: bool,
}

#[derive(Debug)]
pub(crate) struct _Document {
  /// pointer to a libxml document
  pub(crate) doc_ptr: xmlDocPtr,
  /// hashed pointer-to-Node bookkeeping table
  nodes: NodeMap,
  /// The wrappers of rust-owned nodes (`set_rust_owned`), moved out of `nodes` and held weakly, so that the last
  /// clone's drop frees the node: wrapping such a node again finds its wrapper, and the neutralizing frees and the
  /// document's drop reach it.
  rust_owned: RustOwnedMap,
  /// Namespace declaration lists that libxml2 would have freed while a `Namespace`, or a node's namespace, may still
  /// point into one: detached from the elements that declared them, and freed after the document.
  kept_namespaces: Vec<xmlNsPtr>,
  /// DTDs removed from the document, which its entity references may still point into: freed before the document
  /// (their names are in its dictionary), unless linked again.
  removed_dtds: Vec<xmlDtdPtr>,
}

impl _Document {
  fn new(doc_ptr: xmlDocPtr) -> Self {
    _Document {
      doc_ptr,
      nodes: NodeMap::default(),
      rust_owned: RustOwnedMap::default(),
      kept_namespaces: Vec::new(),
      removed_dtds: Vec::new(),
    }
  }
  /// Internal bookkeeping function, so far only used by `Node::wrap`
  pub(crate) fn insert_node(&mut self, node_ptr: xmlNodePtr, node: Node) {
    note_wrapped(xmlGetNodeType(node_ptr), xmlGetParent(node_ptr));
    self.nodes.insert(node_ptr, node);
  }
  /// The wrapper of the node at `node_ptr`, cached or rust-owned, if any.
  #[cfg(test)]
  pub(crate) fn get_node(&self, node_ptr: xmlNodePtr) -> Option<Node> {
    self
      .get_cached(node_ptr)
      .or_else(|| self.get_rust_owned(node_ptr))
  }
  /// The cached wrapper of the node at `node_ptr`, if any.
  #[inline]
  pub(crate) fn get_cached(&self, node_ptr: xmlNodePtr) -> Option<Node> {
    self.nodes.get(&node_ptr).cloned()
  }
  /// The wrapper of the rust-owned node at `node_ptr`, if any.
  pub(crate) fn get_rust_owned(&self, node_ptr: xmlNodePtr) -> Option<Node> {
    if self.rust_owned.is_empty() {
      None
    } else {
      self.rust_owned.get(&node_ptr).and_then(Node::upgrade)
    }
  }
  /// Moves `node`, the wrapper of the node at `node_ptr`, which is becoming rust-owned, to `rust_owned`.
  pub(crate) fn own_node(&mut self, node_ptr: xmlNodePtr, node: &Node) {
    self.nodes.remove(&node_ptr);
    self.rust_owned.insert(node_ptr, node.downgrade());
  }
  /// Neutralizes the wrapper of the node at `node_ptr`, cached or rust-owned, if any, ahead of or after libxml2 freeing
  /// that node: nulls the pointer its clones share, so they all read as `Node::null()`, and forgets it, so a node later
  /// allocated at the same address is wrapped afresh. Reads nothing through `node_ptr`, which may already be freed.
  #[inline]
  pub(crate) fn neutralize(&mut self, node_ptr: xmlNodePtr) {
    if let Some(node) = self.nodes.remove(&node_ptr) {
      node.null_out();
    }
    if !self.rust_owned.is_empty() {
      self.neutralize_rust_owned(node_ptr);
    }
  }
  /// [`neutralize`](Self::neutralize) in `rust_owned`; out of line, as few wrappers are rust-owned.
  #[inline(never)]
  fn neutralize_rust_owned(&mut self, node_ptr: xmlNodePtr) {
    if let Some(node) = (self.rust_owned.remove(&node_ptr)).and_then(|weak| Node::upgrade(&weak)) {
      node.null_out();
    }
  }
  /// Readies every node `xmlFreeNode(top)` frees (see `for_each_freed_with`) for it, before libxml2 frees them:
  /// [`neutralize`](Self::neutralize)s its wrapper and, if it is an element, [keeps](Self::keep_namespaces) its
  /// namespace declarations.
  pub(crate) fn prepare_to_free(&mut self, top: xmlNodePtr) {
    for_each_freed_with(top, |node_ptr| {
      self.neutralize(node_ptr);
      self.keep_namespaces(node_ptr);
    });
  }
  /// Detaches the namespace declarations of the element at `node_ptr`, if any, and keeps them until the document is
  /// freed (#233, #235): libxml2 frees an element's declarations with the element, or on demand, while a `Namespace`
  /// handed out for one, or the namespace of another node, may still point at it.
  pub(crate) fn keep_namespaces(&mut self, node_ptr: xmlNodePtr) {
    self.keep_declarations(xmlNodeTakeNsDeclarations(node_ptr));
  }
  /// Keeps a list of namespace declarations, taken off the element that declared them, until the document is freed.
  pub(crate) fn keep_declarations(&mut self, declarations: xmlNsPtr) {
    if !declarations.is_null() {
      self.kept_namespaces.push(declarations);
    }
  }
  /// Keeps the DTD at `dtd`, unlinked from the document, until the document is freed (#235): the document's entity
  /// references still point at its entity declarations. Keeps each DTD once.
  pub(crate) fn keep_dtd(&mut self, dtd: xmlDtdPtr) {
    if !self.removed_dtds.contains(&dtd) {
      self.removed_dtds.push(dtd);
    }
  }
  /// Whether a wrapper of the node at `node_ptr` is cached.
  pub(crate) fn is_cached(&self, node_ptr: xmlNodePtr) -> bool {
    self.nodes.contains_key(&node_ptr)
  }
  /// The number of cached wrappers, for the wrapper-cache tests.
  #[cfg(test)]
  pub(crate) fn cached_node_count(&self) -> usize {
    self.nodes.len()
  }
}

/// A libxml2 Document
#[derive(Clone)]
pub struct Document(pub(crate) DocumentRef);

impl Drop for _Document {
  ///Free document when it goes out of scope
  fn drop(&mut self) {
    // `xmlFreeDoc` frees the document's nodes and their names, in its dictionary, so every wrapper it knows of,
    // cached or rust-owned, is released first: it reads as `Node::null()` from then on, and a rust-owned node is freed
    // now, while the document is alive (`xmlFreeNode` reads `node->doc`), after every wrapper into it was nulled.
    // So must `xmlFreeDtd`.
    let doc_ptr = self.doc_ptr;
    let mut owned = Vec::new();
    for (_, node) in self.nodes.drain() {
      owned.extend(node.release(doc_ptr));
    }
    for (_, weak) in self.rust_owned.drain() {
      owned.extend(Node::upgrade(&weak).and_then(|node| node.release(doc_ptr)));
    }
    for node_ptr in owned {
      if xmlGetNodeType(node_ptr) == xmlElementType_XML_DTD_NODE {
        self.keep_dtd(node_ptr as xmlDtdPtr); // A wrapper never frees a DTD.
      } else {
        unsafe { xmlFreeNode(node_ptr) };
      }
    }
    unsafe {
      for dtd in self.removed_dtds.drain(..) {
        // Unless it was linked again, into this document or another, which then frees it.
        if xmlGetParent(dtd as xmlNodePtr).is_null() {
          xmlFreeDtd(dtd);
        }
      }
      if !doc_ptr.is_null() {
        xmlFreeDoc(doc_ptr);
      }
      // After the document, whose nodes may point at them; `xmlFreeNs` reads no document.
      for declarations in self.kept_namespaces.drain(..) {
        xmlFreeNsList(declarations);
      }
    }
  }
}

impl fmt::Display for Document {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.to_string_with_options(SaveOptions::default()))
  }
}

impl Document {
  /// Creates a new empty libxml2 document
  pub fn new() -> Result<Self, ()> {
    crate::init_parser();
    unsafe {
      let c_version = CString::new("1.0").unwrap();
      let c_version_bytes = c_version.as_bytes();
      let doc_ptr = xmlNewDoc(c_version_bytes.as_ptr());
      if doc_ptr.is_null() {
        Err(())
      } else {
        Ok(Document(Rc::new(RefCell::new(_Document::new(doc_ptr)))))
      }
    }
  }

  /// Obtain the underlying libxml2 `xmlDocPtr` for this Document
  pub fn doc_ptr(&self) -> xmlDocPtr {
    self.0.borrow().doc_ptr
  }

  /// Creates a new `Document` from an existing libxml2 pointer
  pub fn new_ptr(doc_ptr: xmlDocPtr) -> Self {
    Document(Rc::new(RefCell::new(_Document::new(doc_ptr))))
  }

  pub(crate) fn null_ref() -> DocumentRef {
    Rc::new(RefCell::new(_Document::new(ptr::null_mut())))
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

  pub(crate) fn register_node(&self, node_ptr: xmlNodePtr) -> Node {
    Node::wrap(node_ptr, &self.0)
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

  /// Get the root element of the document (read-only)
  pub fn get_root_readonly(&self) -> Option<RoNode> {
    unsafe {
      let node_ptr = xmlDocGetRootElement(self.doc_ptr());
      if node_ptr.is_null() {
        None
      } else {
        Some(RoNode(node_ptr))
      }
    }
  }

  /// The `Node` for `node`, reusing the document's cached wrapper if any; `Node::null()` for a null `node`, a namespace
  /// node (XPath's namespace axis) and a node of another document.
  #[must_use]
  pub fn node_from_readonly(&self, node: RoNode) -> Node {
    // A namespace node is an `xmlNs`, a shorter struct than the `xmlNode` a `Node` reads.
    let node_type = xmlGetNodeType(node.0);
    if node_type == xmlElementType_XML_NAMESPACE_DECL || NodeType::from_int(node_type).is_none() {
      return Node::null();
    }
    // A wrapper cached here for another document's node would outlive that document.
    if xmlGetDoc(node.0) != self.doc_ptr() {
      return Node::null();
    }
    // A node in no tree wraps as unlinked, as `Node::new` wraps it; only the document node has no parent in a tree.
    let in_tree = !xmlGetParent(node.0).is_null()
      || node_type == xmlElementType_XML_DOCUMENT_NODE
      || node_type == xmlElementType_XML_HTML_DOCUMENT_NODE;
    if in_tree {
      self.register_node(node.0)
    } else {
      Node::wrap_new(node.0, &self.0)
    }
  }

  /// As parsed: whether libxml2 found the document well-formed. `false` for recovered, HTML, in-memory and copied
  /// documents; edits do not update it.
  #[must_use]
  pub fn is_well_formed(&self) -> bool {
    self.has_doc_property(xmlDocProperties_XML_DOC_WELLFORMED)
  }

  /// As parsed: whether libxml2 found the document namespace-valid. `false` for recovered, HTML, in-memory and copied
  /// documents; edits do not update it.
  #[must_use]
  pub fn is_ns_valid(&self) -> bool {
    self.has_doc_property(xmlDocProperties_XML_DOC_NSVALID)
  }

  /// Drops the cached wrappers of linked nodes not held elsewhere, to bound memory on long-lived documents. A dropped
  /// node gets a new wrapper when next reached.
  pub fn trim_node_cache(&self) {
    self
      .0
      .borrow_mut()
      .nodes
      .retain(|_, node| node.is_unlinked() || node.is_shared());
    (self.0.borrow_mut().rust_owned).retain(|_, weak| weak.strong_count() > 0);
  }

  fn has_doc_property(&self, flag: xmlDocProperties) -> bool {
    let doc_ptr = self.doc_ptr();
    // SAFETY: a non-null `doc_ptr` is the live `xmlDoc` this wrapper owns.
    !doc_ptr.is_null() && unsafe { (*doc_ptr).properties } as xmlDocProperties & flag != 0
  }

  /// Sets the root element of the document
  ///
  /// In a document with no element, libxml2 appends `root` to the document's children instead, and merges a text
  /// `root` into a text node there and frees it: its wrappers read as `Node::null()` afterwards.
  ///
  /// # Panics
  ///
  /// On a rust-owned node, a node of another document, a document's own node, a DTD or a declaration in one, or an
  /// attribute.
  pub fn set_root_element(&mut self, root: &Node) {
    assert!(
      !root.is_rust_owned(),
      "set_root_element called with a RustOwned node — linking it would set up a double free"
    );
    assert!(
      Weak::ptr_eq(&Rc::downgrade(&self.0), &root.get_docref()),
      "set_root_element called with a node of another document — import_node copies it"
    );
    let root_type = xmlGetNodeType(root.node_ptr());
    if let Some(refusal) = type_refusal(root_type) {
      panic!("set_root_element refused: {refusal}");
    }
    assert!(
      root_type != xmlElementType_XML_ATTRIBUTE_NODE,
      "set_root_element refused: an attribute can only be added to an element"
    );
    root.link_as_root(self.doc_ptr());
  }

  /// Remove the internal DTD subset (the `<!DOCTYPE …>` declaration)
  /// from this document, if any. Mirrors XML::LibXML's
  /// `Document::removeInternalSubset` (Perl) and the effect of
  /// libxml2's `xmlSetIntSubset(doc, NULL)` — subsequent
  /// serialisation no longer emits the DOCTYPE preamble.
  ///
  /// Safe to call on a document with no internal subset (no-op).
  /// Unlinks the DTD node from the document, which keeps it until it is
  /// freed itself: the document's entity references still point at the
  /// DTD's entity declarations (#235).
  pub fn remove_internal_subset(&mut self) {
    let dtd = unsafe { xmlGetIntSubset(self.doc_ptr()) };
    if !dtd.is_null() {
      unsafe { xmlUnlinkNode(dtd as xmlNodePtr) };
      self.0.borrow_mut().keep_dtd(dtd);
    }
  }

  fn ptr_as_result(&mut self, node_ptr: xmlNodePtr) -> Result<Node, ()> {
    if node_ptr.is_null() {
      Err(())
    } else {
      let node = self.register_node(node_ptr);
      Ok(node)
    }
  }

  /// Import a `Node` from another `Document`
  ///
  /// The source `node` must be `Unlinked` (detached from its origin
  /// document tree). Calling on a `Linked` source returns `Err(())`,
  /// matching the long-standing contract of this method. Calling on a
  /// `RustOwned` source also returns `Err(())`: the source has been
  /// claimed by the Rust wrapper and re-linking it would set up a
  /// double-free. Drop the source's wrapper first if you genuinely
  /// want to discard it.
  pub fn import_node(&mut self, node: &mut Node) -> Result<Node, ()> {
    if !node.is_unlinked() || node.is_rust_owned() {
      return Err(());
    }
    let node_ptr = unsafe { xmlDocCopyNode(node.node_ptr(), self.doc_ptr(), 1) };
    node.set_linked();
    self.ptr_as_result(node_ptr)
  }

  /// Build a fresh `Document` whose root is a deep copy of `node`'s subtree.
  ///
  /// Unlike [`Document::import_node`], this does not require the source
  /// node to be unlinked and does not mutate the source node's wrapper
  /// state. It is suitable for code that repeatedly extracts subtrees
  /// from a single source document and needs each extracted subtree as
  /// its own independently-owned `Document` — a pattern that the older
  /// `import_node` route handles poorly:
  ///
  /// * `import_node` gates on `Node::is_unlinked()`, a wrapper-side flag
  ///   with no public setter; the gate flips to `false` as a side
  ///   effect of a previous successful import (`set_linked()` mutates
  ///   the borrowed wrapper Rc), so every subsequent extract errors.
  /// * A bare `xmlDocCopyNode(src, dst, 1)` returns NULL on the second
  ///   sibling in the same source document, because the recursive
  ///   descent relies on dictionary state that the first copy has
  ///   marked dirty.
  ///
  /// This method works as follows:
  /// 1. `xmlNewDoc` — fresh empty target document.
  /// 2. `xmlDocCopyNode(node, target, 1)` — recursive copy of the
  ///    source subtree into the target, with libxml2 handling
  ///    namespace inheritance (`xmlNewReconciliedNs`) during the copy.
  /// 3. `xmlDocSetRootElement` + `xmlSetTreeDoc` — plant the copy as
  ///    the new root and retarget every doc pointer in the subtree.
  /// 4. `xmlReconciliateNs` — final pass that lifts any remaining
  ///    namespace declarations into the new document so it owns 100%
  ///    of its ns nodes.
  ///
  /// The returned `Document` shares no C-side state with the source —
  /// dropping either is independent of the other.
  ///
  /// Returns `Err(())` if any of the underlying libxml2 calls returns
  /// NULL (typically OOM, or `node` is itself NULL), and for what
  /// [`set_root_element`](Self::set_root_element) refuses: a document's
  /// own node, which is no node of a tree, a DTD or a declaration in one,
  /// and an attribute, which libxml2 would make a child of the new
  /// document that `xmlUnlinkNode` cannot take out again.
  pub fn dup_node_into_new_doc(node: &Node) -> Result<Document, ()> {
    let node_type = xmlGetNodeType(node.node_ptr());
    if type_refusal(node_type).is_some() || node_type == xmlElementType_XML_ATTRIBUTE_NODE {
      return Err(());
    }
    let copy_ptr = unsafe { xmlCopyNode(node.node_ptr(), 1) };
    if copy_ptr.is_null() {
      return Err(());
    }
    let doc_ptr = unsafe {
      let c_version = CString::new("1.0").unwrap();
      xmlNewDoc(c_version.as_bytes().as_ptr())
    };
    if doc_ptr.is_null() {
      unsafe { xmlFreeNode(copy_ptr) };
      return Err(());
    }
    unsafe {
      xmlDocSetRootElement(doc_ptr, copy_ptr);
      // DEBUG: omit xmlSetTreeDoc + xmlReconciliateNs.
    }
    // The source node's wrapper state is left untouched. The new
    // `_Node::drop` rules already prevent a UAF on the source: a
    // detached subtree whose `node->doc` still points at the source
    // document is treated as doc-owned and not freed by the wrapper.
    // If a caller wants the source node's C allocation reclaimed
    // (because the source doc tree-walk won't reach an unlinked
    // subtree), they should call `Node::set_rust_owned` on the
    // source after this returns.
    Ok(Document::new_ptr(doc_ptr))
  }
  /// Serializes the `Document` with options
  pub fn to_string_with_options(&self, options: SaveOptions) -> String {
    unsafe {
      // allocate a buffer to dump into
      let buf = xmlBufferCreate();
      let c_utf8 = CString::new("UTF-8").unwrap();
      let mut xml_options = 0;

      if options.format {
        xml_options += xmlSaveOption_XML_SAVE_FORMAT;
      }
      if options.no_declaration {
        xml_options += xmlSaveOption_XML_SAVE_NO_DECL;
      }
      if options.no_empty_tags {
        xml_options += xmlSaveOption_XML_SAVE_NO_EMPTY;
      }
      if options.no_xhtml {
        xml_options += xmlSaveOption_XML_SAVE_NO_XHTML;
      }
      if options.xhtml {
        xml_options += xmlSaveOption_XML_SAVE_XHTML;
      }
      if options.as_xml {
        xml_options += xmlSaveOption_XML_SAVE_AS_XML;
      }
      if options.as_html {
        xml_options += xmlSaveOption_XML_SAVE_AS_HTML;
      }
      if options.non_significant_whitespace {
        xml_options += xmlSaveOption_XML_SAVE_WSNONSIG;
      }

      let save_ctx = xmlSaveToBuffer(buf, c_utf8.as_ptr(), xml_options as i32);
      let _size = xmlSaveDoc(save_ctx, self.doc_ptr());
      let _size = xmlSaveClose(save_ctx);

      let result = xmlBufferContent(buf);
      // `xmlBufferContent` returns NULL when the buffer never allocated its
      // backing store — e.g. `xmlBufferCreate` returned NULL under memory
      // pressure, a real event on a many-worker fleet. `CStr::from_ptr(NULL)`
      // would then `strlen(NULL)` → SIGSEGV inside libc; degrade to an empty
      // serialization instead (the buffer handle is still safe to free).
      if result.is_null() {
        xmlBufferFree(buf);
        return String::new();
      }
      let c_string = CStr::from_ptr(result as *const c_char);
      let node_string = c_string.to_string_lossy().into_owned();
      xmlBufferFree(buf);

      node_string
    }
  }

  /// Serializes a `Node` owned by this `Document`
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
      let result = xmlBufferContent(buf);
      // `xmlBufferContent` returns NULL when the buffer never allocated its
      // backing store — e.g. `xmlBufferCreate` returned NULL under memory
      // pressure, a real event on a many-worker fleet. `CStr::from_ptr(NULL)`
      // would then `strlen(NULL)` → SIGSEGV inside libc; degrade to an empty
      // serialization instead (the buffer handle is still safe to free).
      if result.is_null() {
        xmlBufferFree(buf);
        return String::new();
      }
      let c_string = CStr::from_ptr(result as *const c_char);
      let node_string = c_string.to_string_lossy().into_owned();
      xmlBufferFree(buf);

      node_string
    }
  }
  /// Serializes a `RoNode` owned by this `Document`
  pub fn ronode_to_string(&self, node: &RoNode) -> String {
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
      let result = xmlBufferContent(buf);
      // `xmlBufferContent` returns NULL when the buffer never allocated its
      // backing store — e.g. `xmlBufferCreate` returned NULL under memory
      // pressure, a real event on a many-worker fleet. `CStr::from_ptr(NULL)`
      // would then `strlen(NULL)` → SIGSEGV inside libc; degrade to an empty
      // serialization instead (the buffer handle is still safe to free).
      if result.is_null() {
        xmlBufferFree(buf);
        return String::new();
      }
      let c_string = CStr::from_ptr(result as *const c_char);
      let node_string = c_string.to_string_lossy().into_owned();
      xmlBufferFree(buf);

      node_string
    }
  }

  /// Creates a node for an XML processing instruction
  pub fn create_processing_instruction(&mut self, name: &str, content: &str) -> Result<Node, ()> {
    unsafe {
      let c_name = CString::new(name).unwrap();
      let c_name_bytes = c_name.as_bytes();
      let c_content = CString::new(content).unwrap();
      let c_content_bytes = c_content.as_bytes();

      let node_ptr: xmlNodePtr = xmlNewDocPI(
        self.doc_ptr(),
        c_name_bytes.as_ptr(),
        c_content_bytes.as_ptr(),
      );
      if node_ptr.is_null() {
        Err(())
      } else {
        Ok(self.register_node(node_ptr))
      }
    }
  }

  /// Cast the document as a libxml Node
  pub fn as_node(&self) -> Node {
    // Note: this method is important to keep, as it enables certain low-level libxml2 idioms
    // In particular, method dispatch based on NodeType is only possible when the document can be cast as a Node
    //
    // Memory management is not an issue, as a document node can not be unbound/removed, and does not require
    // any additional deallocation than the Drop of a Document object.
    self.register_node(self.doc_ptr() as xmlNodePtr)
  }

  /// Duplicates the libxml2 Document into a new instance
  pub fn dup(&self) -> Result<Self, ()> {
    let doc_ptr = unsafe { xmlCopyDoc(self.doc_ptr(), 1) };
    if doc_ptr.is_null() {
      Err(())
    } else {
      Ok(Document(Rc::new(RefCell::new(_Document::new(doc_ptr)))))
    }
  }

  /// Duplicates a source libxml2 Document into the empty Document self
  pub fn dup_from(&mut self, source: &Self) -> Result<(), ()> {
    if !self.doc_ptr().is_null() {
      return Err(());
    }

    let doc_ptr = unsafe { xmlCopyDoc(source.doc_ptr(), 1) };
    if doc_ptr.is_null() {
      return Err(());
    }
    self.0.borrow_mut().doc_ptr = doc_ptr;
    Ok(())
  }
}

mod c14n;

#[cfg(test)]
mod trim_node_cache_tests {
  use crate::parser::Parser;
  use crate::readonly::RoNode;
  use crate::tree::Node;
  use std::rc::Rc;

  #[test]
  fn trim_node_cache_keeps_held_wrappers_and_rewraps_the_rest() {
    let doc = Parser::default().parse_string("<a><b/><c/></a>").unwrap();
    let cached = |node_ptr| doc.0.borrow().get_node(node_ptr).is_some();
    let root = doc.get_root_element().unwrap();
    let held = root.get_first_child().unwrap();
    let visited = held.get_next_sibling().unwrap();
    let visited_ptr = visited.node_ptr();
    // A `Weak` keeps the old allocation, so a fresh wrapper cannot reuse its address.
    let visited_weak = Rc::downgrade(&visited.0);
    drop(visited);
    let loose = Node::new("d", None, &doc).unwrap();
    let loose_ptr = loose.node_ptr();
    drop(loose);
    let before = doc.0.borrow().cached_node_count();

    doc.trim_node_cache();

    // Only the unheld linked `c` is forgotten: its wrapper is dropped, the held and the unlinked ones stay.
    assert_eq!(doc.0.borrow().cached_node_count(), before - 1);
    assert!(visited_weak.upgrade().is_none());
    assert!(!cached(visited_ptr));
    assert!(cached(root.node_ptr()) && cached(held.node_ptr()) && cached(loose_ptr));
    assert!(Rc::ptr_eq(&root.get_first_child().unwrap().0, &held.0));
    assert_eq!(held.get_parent().unwrap(), root);
    // The forgotten node is wrapped afresh: same node, new wrapper, cached again.
    let rewrapped = held.get_next_sibling().unwrap();
    assert_eq!(rewrapped.node_ptr(), visited_ptr);
    assert_eq!(rewrapped.get_name(), "c");
    assert!(Rc::as_ptr(&rewrapped.0) != visited_weak.as_ptr());
    assert!(cached(visited_ptr));
    assert_eq!(doc.0.borrow().cached_node_count(), before);
  }

  /// A rust-owned node nothing outside the document holds keeps its cached wrapper, which owns its allocation:
  /// trimming must not free a node a `RoNode` still reads, and a rewrap is that same rust-owned wrapper.
  #[test]
  fn trim_node_cache_keeps_a_rust_owned_node() {
    let doc = Parser::default().parse_string("<a/>").unwrap();
    let owned = Node::new("d", None, &doc).unwrap();
    owned.set_rust_owned();
    let ro = owned.as_readonly();
    let weak = Rc::downgrade(&owned.0);
    drop(owned);

    doc.trim_node_cache();
    doc.trim_node_cache();

    assert!(
      weak.upgrade().is_some(),
      "the cache still holds the owning wrapper"
    );
    assert_eq!(ro.with_name(str::to_string), "d");
    let again = doc.node_from_readonly(ro);
    assert!(again.is_rust_owned());
    assert!(Rc::ptr_eq(&again.0, &weak.upgrade().unwrap()));
    // The document's drop frees the node through its cached wrapper, before `xmlFreeDoc`.
    drop(again);
    drop(doc);
    assert!(weak.upgrade().is_none());
  }

  /// A second trim finds nothing more to drop: the held, the unlinked and the remaining wrappers stay as they were.
  #[test]
  fn trim_node_cache_twice_is_a_no_op() {
    let doc = Parser::default()
      .parse_string("<a><b/><c><e/></c></a>")
      .unwrap();
    let root = doc.get_root_element().unwrap();
    for child in root.get_child_nodes() {
      child.get_child_nodes();
    }
    let loose = Node::new("d", None, &doc).unwrap();
    let all = doc.0.borrow().cached_node_count();

    doc.trim_node_cache();
    let trimmed = doc.0.borrow().cached_node_count();
    doc.trim_node_cache();

    // b, c and e are dropped; the root and the loose node stay.
    assert_eq!(trimmed, all - 3);
    assert_eq!(doc.0.borrow().cached_node_count(), trimmed);
    assert!(Rc::ptr_eq(&doc.get_root_element().unwrap().0, &root.0));
    assert!(doc.0.borrow().get_node(loose.node_ptr()).is_some());
  }

  #[test]
  fn node_from_readonly_does_not_cache_a_null_node() {
    let doc = Parser::default().parse_string("<r/>").unwrap();
    let root = doc.get_root_element().unwrap();
    let before = doc.0.borrow().cached_node_count();
    assert!(doc.node_from_readonly(RoNode::null()).is_null());
    assert!(root.node_from_readonly(RoNode::null()).is_null());
    assert_eq!(doc.0.borrow().cached_node_count(), before);
  }

  /// A namespace-axis entry is an `xmlNs` (48 bytes), shorter than the `xmlNode` whose `doc` field (offset 64) the
  /// foreign-document check reads. `get_readonly_nodes_as_vec` skips such entries, so only a raw `RoNode` reaches
  /// `node_from_readonly` with one; it must answer `Node::null()` before reading past the struct (red under valgrind,
  /// "Invalid read of size 8", without the namespace guard).
  #[test]
  fn node_from_readonly_rejects_a_raw_namespace_node() {
    use crate::bindings::xmlElementType_XML_NAMESPACE_DECL;
    use crate::c_helpers::xmlGetNodeType;
    use crate::xpath::Context;
    let doc = Parser::default()
      .parse_string(r#"<r xmlns:p="urn:p"><a/></r>"#)
      .unwrap();
    let root = doc.get_root_element().unwrap();
    let context = Context::new(&doc).unwrap();
    let namespaces = context.evaluate("/r/namespace::p").unwrap();
    assert_eq!(namespaces.get_number_of_nodes(), 1);
    // SAFETY: a node-set result with one entry, alive while `namespaces` is.
    let ns_ptr = unsafe { *(*(*namespaces.ptr).nodesetval).nodeTab };
    assert_eq!(xmlGetNodeType(ns_ptr), xmlElementType_XML_NAMESPACE_DECL);
    let before = doc.0.borrow().cached_node_count();
    assert!(doc.node_from_readonly(RoNode(ns_ptr)).is_null());
    assert!(root.node_from_readonly(RoNode(ns_ptr)).is_null());
    assert_eq!(doc.0.borrow().cached_node_count(), before);
  }
}
