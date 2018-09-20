//! The `XPath` functionality

use bindings::*;
use c_helpers::*;
use libc;
use libc::{c_char, c_void, size_t};
use std::ffi::{CStr, CString};
use std::str;
use tree::{Document, DocumentRef, Node};

///The xpath context
#[derive(Clone)]
pub struct Context {
  ///libxml's `ContextPtr`
  pub context_ptr: xmlXPathContextPtr,
  ///Document contains pointer, needed for ContextPtr, so we need to borrow Document to prevent it's freeing
  document: DocumentRef,
}

impl Drop for Context {
  ///free xpath context when it goes out of scope
  fn drop(&mut self) {
    unsafe {
      xmlXPathFreeContext(self.context_ptr);
    }
  }
}

///Essentially, the result of the evaluation of some xpath expression
#[derive(Clone)]
pub struct Object {
  ///libxml's `ObjectPtr`
  pub ptr: xmlXPathObjectPtr,
  document: DocumentRef,
}

impl Context {
  ///create the xpath context for a document
  pub fn new(doc: &Document) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(doc.doc_ptr()) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      Ok(Context {
        context_ptr: ctxtptr,
        document: doc.0.clone(),
      })
    }
  }
  pub(crate) fn new_ptr(docref: DocumentRef) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(docref.borrow().doc_ptr) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      Ok(Context {
        context_ptr: ctxtptr,
        document: docref.clone(),
      })
    }
  }

  /// Instantiate a new Context for the Document of a given Node.
  /// Note: the Context is root-level for that document, use `.set_context_node` to limit scope to this node
  pub fn from_node(node: &Node) -> Result<Context, ()> {
    let docref = node.get_docref().clone();
    Context::new_ptr(docref)
  }

  /// Register a namespace prefix-href pair on the xpath context
  pub fn register_namespace(&self, prefix: &str, href: &str) -> Result<(), ()> {
    let c_prefix = CString::new(prefix).unwrap();
    let c_href = CString::new(href).unwrap();
    unsafe {
      let result = xmlXPathRegisterNs(
        self.context_ptr,
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
    let ptr = unsafe { xmlXPathEvalExpression(c_xpath.as_bytes().as_ptr(), self.context_ptr) };
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
    let ptr = unsafe {
      xmlXPathNodeEval(
        node.node_ptr(),
        c_xpath.as_bytes().as_ptr(),
        self.context_ptr,
      )
    };
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
      let result = xmlXPathSetContextNode(node.node_ptr(), self.context_ptr);
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
    for i in 0..n {
      let ptr = xmlXPathObjectGetNode(self.ptr, i as size_t);
      if ptr.is_null() {
        panic!("rust-libxml: xpath: found null pointer result set");
      }

      let node = Node::wrap(ptr, self.document.clone());
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
