//! Document feature set
//!

use c_signatures::*;
use libc;
use libc::{c_int, c_void};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::ptr;
use std::rc::Rc;
use std::str;

use tree::node::Node;

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
    if !node.is_unlinked() {
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
