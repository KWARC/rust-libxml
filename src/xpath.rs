//! The `XPath` functionality

use crate::bindings::*;
use crate::c_helpers::*;
use crate::tree::{Document, DocumentRef, DocumentWeak, Node};
use libc;
use libc::{c_char, c_void, size_t};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::sync::Arc;
use std::str;

///Thinly wrapped libxml2 xpath context
pub(crate) type ContextRef = Arc<RefCell<_Context>>;

#[derive(Debug)]
pub(crate) struct _Context(pub(crate) xmlXPathContextPtr);

impl Drop for _Context {
  ///free xpath context when it goes out of scope
  fn drop(&mut self) {
    unsafe {
      xmlXPathFreeContext(self.0);
    }
  }
}

/// An XPath context
#[derive(Clone)]
pub struct Context {
  /// Safe reference to the libxml2 context pointer
  pub(crate) context_ptr: ContextRef,
  ///Document contains pointer, needed for ContextPtr, so we need to borrow Document to prevent it's freeing
  pub(crate) document: DocumentWeak,
}

///Essentially, the result of the evaluation of some xpath expression
pub struct Object {
  ///libxml's `ObjectPtr`
  pub ptr: xmlXPathObjectPtr,
  document: DocumentWeak,
}

impl Context {
  ///create the xpath context for a document
  pub fn new(doc: &Document) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(doc.doc_ptr()) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      Ok(Context {
        context_ptr: Arc::new(RefCell::new(_Context(ctxtptr))),
        document: Arc::downgrade(&doc.0),
      })
    }
  }
  pub(crate) fn new_ptr(docref: &DocumentRef) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(docref.borrow().doc_ptr) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      Ok(Context {
        context_ptr: Arc::new(RefCell::new(_Context(ctxtptr))),
        document: Arc::downgrade(&docref),
      })
    }
  }

  /// Returns the raw libxml2 context pointer behind the struct
  pub fn as_ptr(&self) -> xmlXPathContextPtr {
    self.context_ptr.borrow().0
  }

  /// Instantiate a new Context for the Document of a given Node.
  /// Note: the Context is root-level for that document, use `.set_context_node` to limit scope to this node
  pub fn from_node(node: &Node) -> Result<Context, ()> {
    let docref = node.get_docref().upgrade().unwrap();
    Context::new_ptr(&docref)
  }

  /// Register a namespace prefix-href pair on the xpath context
  pub fn register_namespace(&self, prefix: &str, href: &str) -> Result<(), ()> {
    let c_prefix = CString::new(prefix).unwrap();
    let c_href = CString::new(href).unwrap();
    unsafe {
      let result = xmlXPathRegisterNs(
        self.as_ptr(),
        c_prefix.as_bytes().as_ptr(),
        c_href.as_bytes().as_ptr(),
      );
      if result != 0 {
        Err(())
      } else {
        Ok(())
      }
    }
  }

  ///evaluate an xpath
  pub fn evaluate(&self, xpath: &str) -> Result<Object, ()> {
    let c_xpath = CString::new(xpath).unwrap();
    let ptr = unsafe { xmlXPathEvalExpression(c_xpath.as_bytes().as_ptr(), self.as_ptr()) };
    if ptr.is_null() {
      Err(())
    } else {
      Ok(Object {
        ptr,
        document: self.document.clone(),
      })
    }
  }

  ///evaluate an xpath on a context Node
  pub fn node_evaluate(&self, xpath: &str, node: &Node) -> Result<Object, ()> {
    let c_xpath = CString::new(xpath).unwrap();
    let ptr =
      unsafe { xmlXPathNodeEval(node.node_ptr(), c_xpath.as_bytes().as_ptr(), self.as_ptr()) };
    if ptr.is_null() {
      Err(())
    } else {
      Ok(Object {
        ptr,
        document: self.document.clone(),
      })
    }
  }

  /// localize xpath context to a specific Node
  pub fn set_context_node(&mut self, node: &Node) -> Result<(), ()> {
    unsafe {
      let result = xmlXPathSetContextNode(node.node_ptr(), self.as_ptr());
      if result != 0 {
        return Err(());
      }
    }
    Ok(())
  }

  /// find nodes via xpath, at a specified node or the document root
  pub fn findnodes(&mut self, xpath: &str, node_opt: Option<&Node>) -> Result<Vec<Node>, ()> {
    let evaluated;
    if let Some(node) = node_opt {
      evaluated = self.node_evaluate(xpath, node)?;
    } else {
      evaluated = self.evaluate(xpath)?;
    }
    Ok(evaluated.get_nodes_as_vec())
  }

  /// find a literal value via xpath, at a specified node or the document root
  pub fn findvalue(&mut self, xpath: &str, node_opt: Option<&Node>) -> Result<String, ()> {
    let evaluated;
    if let Some(node) = node_opt {
      evaluated = self.node_evaluate(xpath, node)?;
    } else {
      evaluated = self.evaluate(xpath)?;
    }
    Ok(evaluated.to_string())
  }
}

impl Drop for Object {
  /// free the memory allocated
  fn drop(&mut self) {
    unsafe {
      xmlXPathFreeObject(self.ptr);
    }
  }
}

impl Object {
  ///get the number of nodes in the result set
  pub fn get_number_of_nodes(&self) -> usize {
    let v = xmlXPathObjectNumberOfNodes(self.ptr);
    if v == -1 {
      panic!("rust-libxml: xpath: Passed in null pointer!");
    }
    if v == -2 {
      // No nodes found!
      return 0;
    }
    if v < -2 {
      panic!("rust-libxml: xpath: expected non-negative number of result nodes");
    }
    v as usize
  }

  /// returns the result set as a vector of node references
  pub fn get_nodes_as_vec(&self) -> Vec<Node> {
    let n = self.get_number_of_nodes();
    let mut vec: Vec<Node> = Vec::with_capacity(n);
    let slice = if n > 0 {
      xmlXPathObjectGetNodes(self.ptr, n as size_t)
    } else {
      Vec::new()
    };
    for ptr in slice {
      if ptr.is_null() {
        panic!("rust-libxml: xpath: found null pointer result set");
      }
      let node = Node::wrap(ptr, &self.document.upgrade().unwrap());
      vec.push(node);
    }
    vec
  }

  /// use if the XPath used was meant to return a string, such as string(//foo/@attr)
  pub fn to_string(&self) -> String {
    unsafe {
      let receiver = xmlXPathCastToString(self.ptr);
      let c_string = CStr::from_ptr(receiver as *const c_char);
      let rust_string = str::from_utf8(c_string.to_bytes()).unwrap().to_owned();
      libc::free(receiver as *mut c_void);
      rust_string
    }
  }
}
