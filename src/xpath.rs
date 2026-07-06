//! The `XPath` functionality

use crate::bindings::*;
use crate::c_helpers::*;
use crate::error::StructuredError;
use crate::readonly::RoNode;
use crate::tree::{Document, DocumentRef, DocumentWeak, Node};
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
}

///Essentially, the result of the evaluation of some xpath expression
#[derive(Debug)]
pub struct Object {
  ///libxml's `ObjectPtr`
  pub ptr: xmlXPathObjectPtr,
  document: DocumentWeak,
}

/// A structured error raised while evaluating an XPath expression.
///
/// libxml2 aborts an evaluation (returning a NULL result object) for several
/// reasons — most notably when a `//X[predicate]` query has to materialize
/// more than `XPATH_MAX_NODESET_LENGTH` (10,000,000) intermediate nodes and
/// hits the internal *"growing nodeset hit limit"*. The thinly-wrapped
/// [`Context::evaluate`] / [`Context::node_evaluate`] collapse every such
/// failure into a bare `Err(())`, which hides the cause from callers. The
/// `*_checked` variants instead snapshot libxml2's last recorded error into
/// this struct so the real message can be logged or propagated.
#[derive(Debug, Clone)]
pub struct XPathError {
  /// Human-readable message from libxml2 (e.g. `"growing nodeset hit limit"`),
  /// if one was recorded.
  pub message: Option<String>,
  /// libxml2 error code (`xmlParserErrors` enum). `0` when unknown.
  pub code: i32,
  /// libxml2 error domain (`xmlErrorDomain` enum). `0` when unknown.
  pub domain: i32,
}

impl XPathError {
  /// Snapshot the current thread's last libxml2 error — set by libxml2 when an
  /// evaluation returns NULL. Falls back to an empty message when libxml2 did
  /// not record structured detail.
  fn from_last_error() -> Self {
    let err_ptr = unsafe { xmlGetLastError() };
    if err_ptr.is_null() {
      XPathError {
        message: None,
        code: 0,
        domain: 0,
      }
    } else {
      let se = unsafe { StructuredError::from_raw(err_ptr) };
      XPathError {
        message: se.message,
        code: se.code,
        domain: se.domain,
      }
    }
  }

  /// True when this is libxml2's XPath nodeset-length ceiling — the signal that
  /// a `//`-materializing query grew past the 10M-node internal limit (rather
  /// than a syntax error or a missing namespace). The message text is the
  /// reliable discriminator; the underlying code is a generic memory error.
  pub fn is_nodeset_limit(&self) -> bool {
    self
      .message
      .as_deref()
      .is_some_and(|m| m.contains("nodeset") || m.contains("Memory allocation failed"))
  }
}

impl fmt::Display for XPathError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.message {
      Some(m) => write!(
        f,
        "XPath evaluation error: {} (code {}, domain {})",
        m.trim(),
        self.code,
        self.domain
      ),
      None => write!(f, "XPath evaluation failed (no libxml2 error detail)"),
    }
  }
}

impl std::error::Error for XPathError {}

impl Context {
  ///create the xpath context for a document
  pub fn new(doc: &Document) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(doc.doc_ptr()) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      Ok(Context {
        context_ptr: Rc::new(RefCell::new(_Context(ctxtptr))),
        document: Rc::downgrade(&doc.0),
      })
    }
  }
  pub(crate) fn new_ptr(docref: &DocumentRef) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(docref.borrow().doc_ptr) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      Ok(Context {
        context_ptr: Rc::new(RefCell::new(_Context(ctxtptr))),
        document: Rc::downgrade(docref),
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
      if result != 0 { Err(()) } else { Ok(()) }
    }
  }

  ///evaluate an xpath
  pub fn evaluate(&self, xpath: &str) -> Result<Object, ()> {
    self.evaluate_checked(xpath).map_err(|_| ())
  }

  /// Evaluate an XPath expression, returning libxml2's structured error on
  /// failure instead of a bare `()`. This surfaces causes that the thin
  /// [`Context::evaluate`] hides — most importantly the *"growing nodeset hit
  /// limit"* raised when a `//X[predicate]` query materializes more than
  /// `XPATH_MAX_NODESET_LENGTH` intermediate nodes on a very large document.
  pub fn evaluate_checked(&self, xpath: &str) -> Result<Object, XPathError> {
    let c_xpath = CString::new(xpath).unwrap();
    // Reset first so `from_last_error` reads THIS evaluation's error, not a
    // stale one left by an earlier operation on this thread.
    unsafe { xmlResetLastError() };
    let ptr = unsafe { xmlXPathEvalExpression(c_xpath.as_bytes().as_ptr(), self.as_ptr()) };
    if ptr.is_null() {
      Err(XPathError::from_last_error())
    } else {
      Ok(Object {
        ptr,
        document: self.document.clone(),
      })
    }
  }

  ///evaluate an xpath on a context Node
  pub fn node_evaluate(&self, xpath: &str, node: &Node) -> Result<Object, ()> {
    self.node_evaluate_checked(xpath, node).map_err(|_| ())
  }

  /// Evaluate an XPath expression relative to `node`, returning libxml2's
  /// structured error on failure. See [`Context::evaluate_checked`].
  pub fn node_evaluate_checked(&self, xpath: &str, node: &Node) -> Result<Object, XPathError> {
    let c_xpath = CString::new(xpath).unwrap();
    unsafe { xmlResetLastError() };
    let ptr =
      unsafe { xmlXPathNodeEval(node.node_ptr(), c_xpath.as_bytes().as_ptr(), self.as_ptr()) };
    if ptr.is_null() {
      Err(XPathError::from_last_error())
    } else {
      Ok(Object {
        ptr,
        document: self.document.clone(),
      })
    }
  }

  ///evaluate an xpath on a context RoNode
  pub fn node_evaluate_readonly(&self, xpath: &str, node: RoNode) -> Result<Object, ()> {
    self.node_evaluate_readonly_checked(xpath, node).map_err(|_| ())
  }

  /// Evaluate an XPath expression relative to a read-only `node`, returning
  /// libxml2's structured error on failure. See [`Context::evaluate_checked`].
  pub fn node_evaluate_readonly_checked(
    &self,
    xpath: &str,
    node: RoNode,
  ) -> Result<Object, XPathError> {
    let c_xpath = CString::new(xpath).unwrap();
    unsafe { xmlResetLastError() };
    let ptr = unsafe { xmlXPathNodeEval(node.0, c_xpath.as_bytes().as_ptr(), self.as_ptr()) };
    if ptr.is_null() {
      Err(XPathError::from_last_error())
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
      if value_ptr.is_null() {
        // OOM in the cast; record an empty string rather than `strlen(NULL)`.
        vec.push(String::new());
        continue;
      }
      let c_value_string = unsafe { CStr::from_ptr(value_ptr as *const c_char) };
      let ready_str = c_value_string.to_string_lossy().into_owned();
      bindgenFree(value_ptr as *mut c_void);
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
      if receiver.is_null() {
        // OOM in the cast; write nothing rather than `strlen(NULL)`.
        return Ok(());
      }
      let c_string = CStr::from_ptr(receiver as *const c_char);
      let rust_string = str::from_utf8(c_string.to_bytes()).unwrap().to_owned();
      bindgenFree(receiver as *mut c_void);
      write!(f, "{rust_string}")
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
    bindgenFree(xml_xpath_comp_expr_ptr as *mut c_void);
    true
  }
}
