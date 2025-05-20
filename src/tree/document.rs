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
use crate::readonly::RoNode;
use crate::tree::node::Node;

pub(crate) type DocumentRef = Rc<RefCell<_Document>>;
pub(crate) type DocumentWeak = Weak<RefCell<_Document>>;

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
  nodes: HashMap<xmlNodePtr, Node>,
}

impl _Document {
  /// Internal bookkeeping function, so far only used by `Node::wrap`
  pub(crate) fn insert_node(&mut self, node_ptr: xmlNodePtr, node: Node) {
    self.nodes.insert(node_ptr, node);
  }
  /// Internal bookkeeping function, so far only used by `Node::wrap`
  pub(crate) fn get_node(&self, node_ptr: xmlNodePtr) -> Option<&Node> {
    self.nodes.get(&node_ptr)
  }
  /// Internal bookkeeping function
  pub(crate) fn forget_node(&mut self, node_ptr: xmlNodePtr) {
    self.nodes.remove(&node_ptr);
  }
}

/// A libxml2 Document
#[derive(Clone)]
pub struct Document(pub(crate) DocumentRef);

impl Drop for _Document {
  ///Free document when it goes out of scope
  fn drop(&mut self) {
    unsafe {
      if !self.doc_ptr.is_null() {
        xmlFreeDoc(self.doc_ptr);
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
    unsafe {
      let c_version = CString::new("1.0").unwrap();
      let c_version_bytes = c_version.as_bytes();
      let doc_ptr = xmlNewDoc(c_version_bytes.as_ptr());
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

  /// Obtain the underlying libxml2 `xmlDocPtr` for this Document
  pub fn doc_ptr(&self) -> xmlDocPtr {
    self.0.borrow().doc_ptr
  }

  /// Creates a new `Document` from an existing libxml2 pointer
  pub fn new_ptr(doc_ptr: xmlDocPtr) -> Self {
    let doc = _Document {
      doc_ptr,
      nodes: HashMap::new(),
    };
    Document(Rc::new(RefCell::new(doc)))
  }

  pub(crate) fn null_ref() -> DocumentRef {
    Rc::new(RefCell::new(_Document {
      doc_ptr: ptr::null_mut(),
      nodes: HashMap::new(),
    }))
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

  /// Sets the root element of the document
  pub fn set_root_element(&mut self, root: &Node) {
    unsafe {
      xmlDocSetRootElement(self.doc_ptr(), root.node_ptr());
    }
    root.set_linked();
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
  pub fn import_node(&mut self, node: &mut Node) -> Result<Node, ()> {
    if !node.is_unlinked() {
      return Err(());
    }
    // Also remove this node from the prior document hash
    node
      .get_docref()
      .upgrade()
      .unwrap()
      .borrow_mut()
      .forget_node(node.node_ptr());

    let node_ptr = unsafe { xmlDocCopyNode(node.node_ptr(), self.doc_ptr(), 1) };
    node.set_linked();
    self.ptr_as_result(node_ptr)
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
      let doc = _Document {
        doc_ptr,
        nodes: HashMap::new(),
      };
      Ok(Document(Rc::new(RefCell::new(doc))))
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
