//! Streaming pull-parser (`xmlTextReader`).
//!
//! Building the whole DOM for a very large document is prohibitively
//! memory-hungry (a 600 MB file becomes a ~7 GB tree). [`TextReader`] instead
//! streams the input node-by-node and lets callers materialize only the
//! subtrees they care about, so peak memory is one subtree at a time rather
//! than the entire tree.
//!
//! Two ways to materialize the current subtree:
//! * [`TextReader::expand`] — a **borrowed** [`RoNode`], zero-copy, valid only
//!   until the next [`read`](TextReader::read)/[`read_next`](TextReader::read_next).
//!   Ideal for read-only scanning.
//! * [`TextReader::expand_to_document`] — an **owned** [`Document`] copy
//!   (namespaces reconciled), safe to hold, mutate, transform and free after
//!   the reader has advanced. This is the unit the rest of the pipeline
//!   (XSLT, serialization) consumes.
//!
//! ## Streaming a pattern
//!
//! libxml2's XPath engine is not streamable (it needs a fully-built tree). The
//! streamable subset is "downward" name/descendant matching, which at the
//! reader level is simply a per-element name/namespace test — see
//! [`TextReader::read_to_next`]. Arbitrary predicates are then applied on the
//! small owned subtree, where XPath is limit-safe.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use crate::bindings::*;
use crate::readonly::RoNode;
use crate::tree::{Document, NodeType};

/// A safe wrapper over libxml2's `xmlTextReader` pull parser.
///
/// Owns the underlying reader (and the file handle it opened); dropping the
/// `TextReader` frees both.
pub struct TextReader {
  ptr: xmlTextReaderPtr,
}

impl Drop for TextReader {
  fn drop(&mut self) {
    unsafe { xmlFreeTextReader(self.ptr) };
  }
}

/// Borrow a reader-owned `const xmlChar*` (never freed by the caller) as an
/// owned `String`. Returns `None` for a NULL pointer.
fn const_xmlchar_to_string(ptr: *const xmlChar) -> Option<String> {
  if ptr.is_null() {
    return None;
  }
  Some(
    unsafe { CStr::from_ptr(ptr as *const c_char) }
      .to_string_lossy()
      .into_owned(),
  )
}

impl TextReader {
  /// Open `path` for streaming. `options` is the libxml2 parser-option bitmask
  /// (`0` for defaults). Fails if the reader could not be created (e.g. the
  /// file does not exist).
  pub fn from_file(path: &str, options: i32) -> Result<Self, ()> {
    let c_path = CString::new(path).map_err(|_| ())?;
    let ptr = unsafe { xmlReaderForFile(c_path.as_ptr(), ptr::null(), options) };
    if ptr.is_null() {
      Err(())
    } else {
      Ok(TextReader { ptr })
    }
  }

  /// Advance to the next node in document order (descending into children).
  ///
  /// `Ok(true)` = positioned on a node, `Ok(false)` = end of input,
  /// `Err(())` = a parse error occurred.
  pub fn read(&mut self) -> Result<bool, ()> {
    match unsafe { xmlTextReaderRead(self.ptr) } {
      1 => Ok(true),
      0 => Ok(false),
      _ => Err(()),
    }
  }

  /// Advance to the next node that is **not** a descendant of the current node
  /// (i.e. skip the current subtree). Use after materializing a subtree to move
  /// past it without walking its children. Same `Ok(true/false)`/`Err`
  /// semantics as [`read`](Self::read).
  pub fn read_next(&mut self) -> Result<bool, ()> {
    match unsafe { xmlTextReaderNext(self.ptr) } {
      1 => Ok(true),
      0 => Ok(false),
      _ => Err(()),
    }
  }

  /// The current node's type. Returns `None` for reader events that have no
  /// [`NodeType`] equivalent — most usefully the *end-of-element* event, which
  /// lets a caller distinguish an opening `<x>` (`Some(ElementNode)`) from a
  /// closing `</x>`.
  pub fn node_type(&self) -> Option<NodeType> {
    // xmlTextReaderNodeType returns an xmlReaderTypes value; for the shared
    // cases (element/text/PI/comment/cdata) it is numerically identical to the
    // xmlElementType NodeType::from_int expects.
    NodeType::from_int(unsafe { xmlTextReaderNodeType(self.ptr) } as xmlElementType)
  }

  /// True when positioned on an element *start* tag.
  pub fn is_element(&self) -> bool { self.node_type() == Some(NodeType::ElementNode) }

  /// The current node's depth in the tree (root element = 0).
  pub fn depth(&self) -> i32 { unsafe { xmlTextReaderDepth(self.ptr) } }

  /// The current node's local name (no namespace prefix), if any.
  pub fn local_name(&self) -> Option<String> {
    const_xmlchar_to_string(unsafe { xmlTextReaderConstLocalName(self.ptr) })
  }

  /// The current node's namespace URI, if any.
  pub fn namespace_uri(&self) -> Option<String> {
    const_xmlchar_to_string(unsafe { xmlTextReaderConstNamespaceUri(self.ptr) })
  }

  /// Fully build the current node's subtree and borrow it read-only.
  ///
  /// Zero-copy. **The returned [`RoNode`] is owned by the reader and is
  /// invalidated by the next [`read`](Self::read)/[`read_next`](Self::read_next)** — do
  /// not retain it across an advance. For a subtree you can keep, use
  /// [`expand_to_document`](Self::expand_to_document). Returns `None` at end of
  /// input or on error.
  pub fn expand(&self) -> Option<RoNode> {
    let node = unsafe { xmlTextReaderExpand(self.ptr) };
    if node.is_null() {
      None
    } else {
      Some(RoNode(node))
    }
  }

  /// Copy the current node's subtree into a fresh, independently-owned
  /// [`Document`] whose root element is the copy.
  ///
  /// Namespaces declared on un-copied ancestors (e.g. the default `xmlns` on
  /// the real document root) are reconciled onto the copy via
  /// `xmlDOMWrapCloneNode`, so the result is self-contained — safe to hold,
  /// mutate, transform and serialize after the reader has advanced and freed
  /// its own copy of the subtree. Returns `None` at end of input or on error.
  pub fn expand_to_document(&self) -> Option<Document> {
    let node = unsafe { xmlTextReaderExpand(self.ptr) };
    if node.is_null() {
      return None;
    }
    unsafe {
      let newdoc = xmlNewDoc(c"1.0".as_ptr() as *const xmlChar);
      if newdoc.is_null() {
        return None;
      }
      // xmlDOMWrapCloneNode (unlike xmlDocCopyNode) reconciles namespaces from
      // the source ancestors onto the clone, so the detached subtree does not
      // dangle into the source document once the reader frees it. A wrap
      // context is required for the reconciliation to actually *declare* the
      // in-scope namespaces on the clone (with a NULL context the clone keeps
      // an ns pointer but the `xmlns=` decl is not materialized, so
      // serialization silently drops it).
      let ctxt = xmlDOMWrapNewCtxt();
      let mut cloned: xmlNodePtr = ptr::null_mut();
      let src_doc = (*node).doc;
      let rc = xmlDOMWrapCloneNode(
        ctxt, src_doc, node, &mut cloned, newdoc,
        ptr::null_mut(), // no destination parent — it becomes the root
        1,               // deep
        0,               // options
      );
      xmlDOMWrapFreeCtxt(ctxt);
      if rc != 0 || cloned.is_null() {
        xmlFreeDoc(newdoc);
        return None;
      }
      xmlDocSetRootElement(newdoc, cloned);
      // Belt-and-suspenders: ensure every namespace used in the detached tree
      // is declared within it (self-contained serialization, no dangling ns).
      xmlReconciliateNs(newdoc, cloned);
      Some(Document::new_ptr(newdoc))
    }
  }

  /// Advance until positioned on the next element whose `(namespace, localname)`
  /// satisfies `want`, or the end of input.
  ///
  /// This is the streaming analogue of a downward `//name` XPath step: the only
  /// XPath subset that is actually streamable. Returns `Ok(true)` when
  /// positioned on a match (then call [`expand`](Self::expand) /
  /// [`expand_to_document`](Self::expand_to_document), and
  /// [`read_next`](Self::read_next) to skip past it), `Ok(false)` at end of input.
  ///
  /// `want` receives the namespace URI (`None` if the element is in no
  /// namespace) and the local name.
  pub fn read_to_next<F>(&mut self, want: F) -> Result<bool, ()>
  where
    F: Fn(Option<&str>, &str) -> bool,
  {
    while self.read()? {
      if self.is_element()
        && let Some(name) = self.local_name()
        && want(self.namespace_uri().as_deref(), &name)
      {
        return Ok(true);
      }
    }
    Ok(false)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const NS: &str = "http://example.org/ns";

  fn write_temp(name: &str, xml: &str) -> String {
    let path = std::env::temp_dir().join(format!("rust-libxml-reader-{}-{name}.xml", std::process::id()));
    std::fs::write(&path, xml).unwrap();
    path.to_string_lossy().into_owned()
  }

  /// Stream a multi-section document, collect each `<section>` as an owned
  /// Document, and verify — crucially, *after the reader is dropped* — that the
  /// copies are self-contained: namespaces inherited from the (un-copied) root
  /// are reconciled onto each copy, and content survives.
  #[test]
  fn stream_sections_owned_and_namespace_reconciled() {
    let xml = r#"<?xml version="1.0"?>
<doc xmlns="http://example.org/ns" xmlns:x="http://example.org/x">
  <meta>skip me</meta>
  <section id="a"><title>Alpha</title><p>one</p></section>
  <section id="b"><title>Beta</title><x:note>hi</x:note></section>
</doc>"#;
    let path = write_temp("sections", xml);

    let mut sections = Vec::new();
    {
      let mut reader = TextReader::from_file(&path, 0).unwrap();
      while reader
        .read_to_next(|ns, name| ns == Some(NS) && name == "section")
        .unwrap()
      {
        sections.push(reader.expand_to_document().unwrap());
      }
      // reader dropped here — its copies of the subtrees are freed.
    }

    assert_eq!(sections.len(), 2, "should stream exactly two <section>s");

    // Root of each owned doc is a <section> in the reconciled default ns.
    let root0 = sections[0].get_root_element().unwrap();
    assert_eq!(root0.get_name(), "section");
    assert_eq!(root0.get_attribute("id").as_deref(), Some("a"));
    assert_eq!(
      root0.get_namespace().map(|n| n.get_href()),
      Some(NS.to_string()),
      "default namespace must be reconciled onto the detached copy"
    );

    // Serialization is intact and namespace-declared (no dangling ns → no UAF).
    let s0 = sections[0].to_string();
    assert!(s0.contains("http://example.org/ns"), "ns decl missing: {s0}");
    assert!(s0.contains("Alpha") && s0.contains("one"), "content lost: {s0}");

    // The second section keeps its prefixed namespace too.
    let s1 = sections[1].to_string();
    assert!(s1.contains("Beta"), "content lost: {s1}");
    assert!(s1.contains("http://example.org/x"), "prefixed ns lost: {s1}");

    std::fs::remove_file(&path).ok();
  }

  /// `read`/`next`/`is_element`/`local_name` walk the tree and `read_next` skips a
  /// subtree (does not descend).
  #[test]
  fn read_and_next_skip_subtree() {
    let xml = r#"<r><a><deep/></a><b/></r>"#;
    let path = write_temp("skip", xml);
    let mut reader = TextReader::from_file(&path, 0).unwrap();

    assert!(reader.read().unwrap()); // <r>
    assert!(reader.is_element());
    assert_eq!(reader.local_name().as_deref(), Some("r"));

    assert!(reader.read().unwrap()); // <a>
    assert_eq!(reader.local_name().as_deref(), Some("a"));

    // read_next() skips <a>'s subtree (the <deep/>) → lands on <b>.
    assert!(reader.read_next().unwrap());
    assert_eq!(reader.local_name().as_deref(), Some("b"));

    std::fs::remove_file(&path).ok();
  }
}
