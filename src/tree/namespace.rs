//! Namespace feature set
//!
use std::error::Error;
use std::ffi::{CStr, CString};
use std::hash::{Hash, Hasher};
use std::panic::AssertUnwindSafe;
use std::ptr;
use std::rc::Rc;
use std::str;

use crate::bindings::*;
use crate::c_helpers::*;
use crate::tree::document::DocumentRef;
use crate::tree::node::Node;

/// An xml namespace
///
/// A namespace points into its document. One from a [`Node`], from [`Namespace::new`] or from
/// `RoNode::get_namespaces` (given the node's document) keeps the document alive, as an XPath `Context` does. One from
/// any other `RoNode` method does not, since an `RoNode` does not know its `Document`: it must not outlive the document,
/// as before. The methods that set a namespace on a node refuse one of another document, which would point into that
/// document after it drops.
///
/// Known caveat, as for an `RoNode` itself: a namespace from `RoNode::get_namespace`, `get_namespace_declarations` or
/// `get_properties_ns` that outlives its document points into freed memory, and the refusal above, which compares
/// document addresses, may take it for one of a later document at the same address. Lifetimes are to rule this out in
/// 0.4, with the `RoNode` lifetime of #239.
pub struct Namespace {
  ///libxml's xmlNsPtr
  pub(crate) ns_ptr: xmlNsPtr,
  /// The libxml2 document the namespace is in: the document of the node it came from.
  doc_ptr: xmlDocPtr,
  /// The document of the namespace, kept alive with it; `None` from an `RoNode`, which does not know its `Document`
  /// (except `RoNode::get_namespaces`, given it).
  /// (`AssertUnwindSafe`: a document's `RefCell` is not `RefUnwindSafe`, which `Namespace` has always been.)
  document: Option<AssertUnwindSafe<DocumentRef>>,
}

impl Clone for Namespace {
  fn clone(&self) -> Self {
    Namespace {
      ns_ptr: self.ns_ptr,
      doc_ptr: self.doc_ptr,
      document: self
        .document
        .as_ref()
        .map(|document| AssertUnwindSafe(Rc::clone(&document.0))),
    }
  }
}

impl PartialEq for Namespace {
  fn eq(&self, other: &Self) -> bool {
    self.get_prefix() == other.get_prefix() && self.get_href() == other.get_href()
  }
}

impl Eq for Namespace {}

impl Hash for Namespace {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.get_prefix().hash(state);
    self.get_href().hash(state);
  }
}

impl Namespace {
  /// The namespace at `ns_ptr`, in the libxml2 document `doc_ptr`, whose `Document` is `document` when it is known.
  pub(crate) fn of(ns_ptr: xmlNsPtr, doc_ptr: xmlDocPtr, document: Option<DocumentRef>) -> Self {
    Namespace {
      ns_ptr,
      doc_ptr,
      document: document.map(AssertUnwindSafe),
    }
  }

  /// Whether the namespace may be set on a node of the libxml2 document `doc_ptr`: it is in that document. A node in
  /// no document (`doc_ptr` null) takes none, since it would point into the namespace's document after that drops.
  #[inline]
  pub(crate) fn may_be_used_in(&self, doc_ptr: xmlDocPtr) -> bool {
    !doc_ptr.is_null() && self.doc_ptr == doc_ptr
  }

  /// Creates a new namespace
  pub fn new(
    prefix: &str,
    href: &str,
    node: &mut Node,
  ) -> Result<Self, Box<dyn Error + Send + Sync>> {
    let c_href = CString::new(href)?;
    let c_prefix = CString::new(prefix)?;
    let c_prefix_ptr = if prefix.is_empty() {
      ptr::null()
    } else {
      c_prefix.as_ptr()
    };

    let node_ptr = node.node_ptr_mut()?;
    unsafe {
      let ns = xmlNewNs(
        node_ptr,
        c_href.as_bytes().as_ptr(),
        c_prefix_ptr as *const u8,
      );
      if ns.is_null() {
        Err(From::from("xmlNewNs returned NULL"))
      } else {
        Ok(Namespace::of(
          ns,
          xmlGetDoc(node_ptr),
          node.upgrade_docref(),
        ))
      }
    }
  }

  /// Immutably borrows the underlying libxml2 `xmlNsPtr` pointer
  pub fn ns_ptr(&self) -> xmlNsPtr {
    self.ns_ptr
  }

  /// Mutably borrows the underlying libxml2 `xmlNsPtr` pointer
  pub fn ns_ptr_mut(&mut self) -> xmlNsPtr {
    self.ns_ptr
  }
  /// The namespace prefix
  pub fn get_prefix(&self) -> String {
    prefix_of(self.ns_ptr)
  }

  /// The namespace href
  pub fn get_href(&self) -> String {
    href_of(self.ns_ptr)
  }

  /// Does nothing. A namespace belongs to its document: libxml2 frees it with the element that declares it, or the
  /// document keeps it until it is freed itself. This used to free it, while its element still declared it, so that
  /// libxml2 freed it a second time (#233).
  #[deprecated(
    since = "0.3.25",
    note = "a namespace is freed with its document; this does nothing"
  )]
  pub fn free(&mut self) {}
}

/// The prefix of the namespace at `ns_ptr`, `""` for none.
pub(crate) fn prefix_of(ns_ptr: xmlNsPtr) -> String {
  c_string_or_empty(xmlNsPrefix(ns_ptr))
}

/// The URI of the namespace at `ns_ptr`, `""` for none.
pub(crate) fn href_of(ns_ptr: xmlNsPtr) -> String {
  c_string_or_empty(xmlNsHref(ns_ptr))
}

fn c_string_or_empty(c_ptr: *const std::os::raw::c_char) -> String {
  if c_ptr.is_null() {
    String::new()
  } else {
    unsafe { CStr::from_ptr(c_ptr) }
      .to_string_lossy()
      .into_owned()
  }
}
