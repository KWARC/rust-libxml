//! The `XPath` functionality

use crate::{
  bindings::{self, *},
  c_helpers::*,
  error::StructuredError,
  readonly::RoNode,
  schemas::structured_error_handler,
  tree::{Document, DocumentRef, DocumentWeak, Node},
};
use libc::{c_char, c_void, size_t};
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::fmt;
use std::rc::Rc;
use std::str;

///Thinly wrapped libxml2 xpath context
pub(crate) type ContextRef = Rc<RefCell<_Context>>;

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
  ///Errors registered during libxml2 xpath processing3
  pub(crate) errlog: *mut Vec<StructuredError>,
}

impl Drop for Context {
  fn drop(&mut self) {
    unsafe {
      if !self.errlog.is_null() {
        let errors: Box<Vec<StructuredError>> = std::mem::transmute(self.errlog);
        drop(errors)
      }
    }
  }
}

///Essentially, the result of the evaluation of some xpath expression
#[derive(Debug)]
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
      let errors: Box<Vec<StructuredError>> = Box::default();

      unsafe {
        let reference: *mut Vec<StructuredError> = std::mem::transmute(errors);
        bindings::xmlXPathSetErrorHandler(
          ctxtptr,
          Some(structured_error_handler),
          reference as *mut _,
        );
        Ok(Context {
          context_ptr: Rc::new(RefCell::new(_Context(ctxtptr))),
          document: Rc::downgrade(&doc.0),
          errlog: reference as *mut _,
        })
      }
    }
  }

  pub(crate) fn new_ptr(docref: &DocumentRef) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(docref.borrow().doc_ptr) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      let errors: Box<Vec<StructuredError>> = Box::default();

      unsafe {
        let reference: *mut Vec<StructuredError> = std::mem::transmute(errors);
        bindings::xmlXPathSetErrorHandler(
          ctxtptr,
          Some(structured_error_handler),
          reference as *mut _,
        );

        Ok(Context {
          context_ptr: Rc::new(RefCell::new(_Context(ctxtptr))),
          document: Rc::downgrade(docref),
          errlog: reference as *mut _,
        })
      }
    }
  }

  /// Returns the raw libxml2 context pointer behind the struct
  pub fn as_ptr(&self) -> xmlXPathContextPtr {
    self.context_ptr.borrow().0
  }

  /// Drains error log from errors that might have accumulated while evaluating an xpath
  pub fn drain_errors(&mut self) -> Vec<StructuredError> {
    assert!(!self.errlog.is_null());
    let errors = unsafe { &mut *self.errlog };
    std::mem::take(errors)
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

  ///evaluate an xpath on a context RoNode
  pub fn node_evaluate_readonly(&self, xpath: &str, node: RoNode) -> Result<Object, ()> {
    let c_xpath = CString::new(xpath).unwrap();
    let ptr = unsafe { xmlXPathNodeEval(node.0, c_xpath.as_bytes().as_ptr(), self.as_ptr()) };
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
    let evaluated = if let Some(node) = node_opt {
      self.node_evaluate(xpath, node)?
    } else {
      self.evaluate(xpath)?
    };
    Ok(evaluated.get_nodes_as_vec())
  }

  /// find literal values via xpath, at a specified node or the document root
  pub fn findvalues(&mut self, xpath: &str, node_opt: Option<&Node>) -> Result<Vec<String>, ()> {
    let evaluated = if let Some(node) = node_opt {
      self.node_evaluate(xpath, node)?
    } else {
      self.evaluate(xpath)?
    };
    Ok(evaluated.get_nodes_as_str())
  }

  /// find a literal value via xpath, at a specified node or the document root
  pub fn findvalue(&mut self, xpath: &str, node_opt: Option<&Node>) -> Result<String, ()> {
    let evaluated = if let Some(node) = node_opt {
      self.node_evaluate(xpath, node)?
    } else {
      self.evaluate(xpath)?
    };
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

  /// returns the result set as a vector of `Node` objects
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

  /// returns the result set as a vector of `RoNode` objects
  pub fn get_readonly_nodes_as_vec(&self) -> Vec<RoNode> {
    let n = self.get_number_of_nodes();
    let mut vec: Vec<RoNode> = Vec::with_capacity(n);
    let slice = if n > 0 {
      xmlXPathObjectGetNodes(self.ptr, n as size_t)
    } else {
      Vec::new()
    };
    for ptr in slice {
      if ptr.is_null() {
        panic!("rust-libxml: xpath: found null pointer result set");
      }
      vec.push(RoNode(ptr));
    }
    vec
  }

  /// returns the result set as a vector of Strings
  pub fn get_nodes_as_str(&self) -> Vec<String> {
    let n = self.get_number_of_nodes();
    let mut vec: Vec<String> = Vec::with_capacity(n);
    let slice = if n > 0 {
      xmlXPathObjectGetNodes(self.ptr, n as size_t)
    } else {
      Vec::new()
    };
    for ptr in slice {
      if ptr.is_null() {
        panic!("rust-libxml: xpath: found null pointer result set");
      }
      let value_ptr = unsafe { xmlXPathCastNodeToString(ptr) };
      let c_value_string = unsafe { CStr::from_ptr(value_ptr as *const c_char) };
      let ready_str = c_value_string.to_string_lossy().into_owned();
      unsafe {
        libc::free(value_ptr as *mut c_void);
      }
      vec.push(ready_str);
    }
    vec
  }
}

impl fmt::Display for Object {
  /// use if the XPath used was meant to return a string, such as string(//foo/@attr)
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    unsafe {
      let receiver = xmlXPathCastToString(self.ptr);
      let c_string = CStr::from_ptr(receiver as *const c_char);
      let rust_string = str::from_utf8(c_string.to_bytes()).unwrap().to_owned();
      libc::free(receiver as *mut c_void);
      write!(f, "{}", rust_string)
    }
  }
}

/// Calls the binding to http://xmlsoft.org/html/libxml-xpath.html#xmlXPathCompile and return true if
/// a non-null pointer is returned. The idea is to use this to validate an xpath independent of context.
/// Tests describing what this validates in tests/xpath_tests.rs
pub fn is_well_formed_xpath(xpath: &str) -> bool {
  let c_xpath = CString::new(xpath).unwrap();
  let xml_xpath_comp_expr_ptr = unsafe { xmlXPathCompile(c_xpath.as_bytes().as_ptr()) };
  if xml_xpath_comp_expr_ptr.is_null() {
    false
  } else {
    unsafe {
      libc::free(xml_xpath_comp_expr_ptr as *mut c_void);
    }
    true
  }
}
