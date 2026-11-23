//! The `XPath` functionality

use crate::bindings::*;
use crate::c_helpers::*;
use crate::error::StructuredError;
use crate::readonly::RoNode;
use crate::tree::document::{_Document, unwatch_node_set_on_thread};
use crate::tree::{Document, DocumentRef, Node};
use libc::{c_char, c_int, c_void, size_t};
use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString};
use std::fmt;
use std::rc::Rc;
use std::str;

///Thinly wrapped libxml2 xpath context
pub(crate) type ContextRef = Rc<RefCell<_Context>>;

#[derive(Debug)]
pub(crate) struct _Context {
  pub(crate) ptr: xmlXPathContextPtr,
  /// The context node an evaluation at a node or `set_context_node` left, kept so that the context forgets it once it
  /// is freed (see `forget_freed_context_node`). Dropped after the libxml2 context is freed.
  node: ContextNode,
}

/// How a `_Context` learns that its context node was freed.
#[derive(Debug)]
enum ContextNode {
  None,
  /// The `Node` passed in, which a free of its node neutralizes.
  Wrapped(Node),
  /// An `RoNode`'s node, of `document`, which logs the frees while this watches it (as an XPath node set does, see
  /// `_Document::watch_node_set`) after `seen` frees: no wrapper is made for it.
  Readonly {
    ptr: xmlNodePtr,
    document: DocumentRef,
    seen: u64,
  },
}

impl _Context {
  fn new(ptr: xmlXPathContextPtr) -> Self {
    _Context {
      ptr,
      node: ContextNode::None,
    }
  }

  /// Ends a `ContextNode::Readonly`'s watch of its document, and keeps `node` instead.
  fn replace_node(&mut self, node: ContextNode) {
    if let ContextNode::Readonly { document, .. } = std::mem::replace(&mut self.node, node) {
      unwatch(&document);
    }
  }
}

impl Drop for _Context {
  ///free xpath context when it goes out of scope
  fn drop(&mut self) {
    unsafe {
      xmlXPathFreeContext(self.ptr);
    }
    self.replace_node(ContextNode::None);
  }
}

/// Ends a watch of `document` (`_Document::unwatch_node_set`). If the document is borrowed, which no method leaves it
/// while a context or an object drops, only the thread's count ends: the document then logs frees until it drops.
fn unwatch(document: &DocumentRef) {
  match document.try_borrow_mut() {
    Ok(mut doc) => doc.unwatch_node_set(),
    Err(_) => unwatch_node_set_on_thread(),
  }
}

/// An XPath context
///
/// It keeps its document alive, as a [`Document`] handle does (#61): libxml2 evaluates against the document, and the
/// document holds no context, so no cycle forms.
///
/// An evaluation at a node, or [`set_context_node`](Context::set_context_node), leaves that node as the context node,
/// for the evaluations without one. If the node is freed, the context forgets it, and evaluates as a new context does,
/// at no node.
#[derive(Clone)]
pub struct Context {
  /// Safe reference to the libxml2 context pointer
  ///
  /// Declared before `document`, so that the last clone frees the libxml2 context before it lets go of the document.
  pub(crate) context_ptr: ContextRef,
  /// The document the context evaluates against, kept alive with it.
  pub(crate) document: DocumentRef,
}

///Essentially, the result of the evaluation of some xpath expression
///
/// It keeps its document alive, as its [`Context`] does: its node set points into the document. A node of the set that
/// is freed while the `Object` is alive (by `free_subtree`, `set_content` on an ancestor, `remove_property`, a text
/// merge, and so on) is left out of the set from then on.
pub struct Object {
  ///libxml's `ObjectPtr`
  pub ptr: xmlXPathObjectPtr,
  /// The document of the node set, let go of after `Drop::drop` frees `ptr`.
  document: DocumentRef,
  /// For a non-empty node set, the count of frees its document had logged when the set last left out the freed nodes
  /// (see `_Document::watch_node_set`).
  frees_seen: Cell<Option<u64>>,
}

impl fmt::Debug for Object {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Object")
      .field("ptr", &self.ptr)
      .finish_non_exhaustive()
  }
}

/// A structured error from an XPath evaluation.
///
/// libxml2 returns a NULL result — which the bare [`Context::evaluate`] family
/// collapses to `Err(())` — for several reasons, most importantly the
/// *"growing nodeset hit limit"* when a `//X[predicate]` query materializes
/// more than `XPATH_MAX_NODESET_LENGTH` (10M) nodes on a huge document. The
/// `*_checked` variants snapshot libxml2's last error here so the cause is
/// recoverable; see [`is_nodeset_limit`](Self::is_nodeset_limit).
#[derive(Debug, Clone)]
pub struct XPathError {
  /// libxml2's message (e.g. `"growing nodeset hit limit"`), if recorded.
  pub message: Option<String>,
  /// libxml2 error code (`xmlParserErrors`). `0` when unknown.
  pub code: i32,
  /// libxml2 error domain (`xmlErrorDomain`). `0` when unknown.
  pub domain: i32,
}

impl XPathError {
  /// Snapshot the thread's last libxml2 error (set when an evaluation returns
  /// NULL); empty message when libxml2 recorded no structured detail.
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

/// A compiled XPath expression (libxml2's `xmlXPathCompExpr`): parsed once, then evaluated any number of times
/// without the compilation that every string-taking evaluator ([`Context::evaluate`], [`Context::findnodes`], …)
/// repeats on each call. Evaluating it gives the result of evaluating its source string: libxml2's string evaluators
/// compile, optimize and run the expression exactly as [`Expression::compile`] and [`Context::evaluate_compiled`] do.
///
/// An expression holds no document and no context, so one `Expression` may be evaluated against any [`Context`].
/// Two caveats are libxml2's own: a namespace prefix is resolved when the expression runs, from the context it runs
/// in; and the first evaluation of a function call caches the function it found, so an expression calling an
/// extension function should only run in contexts that register that same function. Because evaluation writes that
/// cache into the expression, an `Expression` is neither `Send` nor `Sync`.
#[derive(Debug)]
pub struct Expression {
  ptr: xmlXPathCompExprPtr,
}

impl Expression {
  /// Compile `xpath`, or return libxml2's structured error when it is not a well-formed expression.
  pub fn compile(xpath: &str) -> Result<Expression, XPathError> {
    crate::init_parser();
    let c_xpath = CString::new(xpath).map_err(|_| XPathError {
      message: Some("XPath expression contains a NUL byte".to_string()),
      code: 0,
      domain: 0,
    })?;
    unsafe { xmlResetLastError() };
    let ptr = unsafe { xmlXPathCompile(c_xpath.as_bytes().as_ptr()) };
    if ptr.is_null() {
      Err(XPathError::from_last_error())
    } else {
      Ok(Expression { ptr })
    }
  }

  /// Returns the raw libxml2 compiled-expression pointer behind the struct
  pub fn as_ptr(&self) -> xmlXPathCompExprPtr {
    self.ptr
  }
}

impl Drop for Expression {
  /// free the compiled expression, with its steps, when it goes out of scope
  fn drop(&mut self) {
    unsafe { xmlXPathFreeCompExpr(self.ptr) };
  }
}

impl Context {
  ///create the xpath context for a document
  pub fn new(doc: &Document) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(doc.doc_ptr()) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      Ok(Context {
        context_ptr: Rc::new(RefCell::new(_Context::new(ctxtptr))),
        document: Rc::clone(&doc.0),
      })
    }
  }
  pub(crate) fn new_ptr(docref: &DocumentRef) -> Result<Context, ()> {
    let ctxtptr = unsafe { xmlXPathNewContext(docref.borrow().doc_ptr) };
    if ctxtptr.is_null() {
      Err(())
    } else {
      Ok(Context {
        context_ptr: Rc::new(RefCell::new(_Context::new(ctxtptr))),
        document: Rc::clone(docref),
      })
    }
  }

  /// Returns the raw libxml2 context pointer behind the struct
  pub fn as_ptr(&self) -> xmlXPathContextPtr {
    self.context_ptr.borrow().ptr
  }

  /// Keeps `node` as the context node, if libxml2 left it as the context node (it does not when it refuses it, such as
  /// a node of another document).
  fn keep_context_node(&self, node: &Node) {
    let node_ptr = node.node_ptr();
    let mut context = self.context_ptr.borrow_mut();
    // SAFETY: `context.ptr` is a live XPath context.
    if node_ptr.is_null() || unsafe { (*context.ptr).node } != node_ptr {
      return;
    }
    if !matches!(&context.node, ContextNode::Wrapped(kept) if kept.node_ptr() == node_ptr) {
      context.replace_node(ContextNode::Wrapped(node.clone()));
    }
  }

  /// Keeps the node of `node` as the context node, as `keep_context_node` does, without wrapping it: the document logs
  /// the frees while the context keeps it.
  fn keep_readonly_context_node(&self, node: RoNode) {
    let mut context = self.context_ptr.borrow_mut();
    // SAFETY: `context.ptr` is a live XPath context.
    if node.0.is_null() || unsafe { (*context.ptr).node } != node.0 {
      return;
    }
    if let ContextNode::Readonly {
      ptr,
      document,
      seen,
    } = &mut context.node
    {
      // Already watching: the frees before now are of no concern to the new node.
      *ptr = node.0;
      *seen = document.borrow_mut().frees_seen();
    } else {
      let seen = self.document.borrow_mut().watch_node_set();
      context.replace_node(ContextNode::Readonly {
        ptr: node.0,
        document: self.document.clone(),
        seen,
      });
    }
  }

  /// Forgets the context node, if it was freed (its kept wrapper was neutralized, or its document logged its free):
  /// libxml2 would read it.
  fn forget_freed_context_node(&self) {
    let mut context = self.context_ptr.borrow_mut();
    let freed = match &mut context.node {
      ContextNode::None => false,
      ContextNode::Wrapped(node) => node.is_null(),
      ContextNode::Readonly {
        ptr,
        document,
        seen,
      } => {
        let mut doc = document.borrow_mut();
        if doc.frees_logged() == *seen {
          false
        } else if doc.freed_since(*seen).contains(ptr) {
          true
        } else {
          *seen = doc.frees_seen();
          false
        }
      }
    };
    if freed {
      // SAFETY: `context.ptr` is a live XPath context.
      unsafe { (*context.ptr).node = std::ptr::null_mut() };
      context.replace_node(ContextNode::None);
    }
  }

  /// The `Object` for `ptr`, a result of evaluating in this context, not yet [`watched`](Object::watched): an
  /// evaluator that hands it out watches it; one that reads it before any node can be freed, as `findnodes` does, need
  /// not.
  fn object(&self, ptr: xmlXPathObjectPtr) -> Object {
    Object::unwatched(ptr, self.document.clone())
  }

  /// Instantiate a new Context for the Document of a given Node.
  /// Note: the Context is root-level for that document, use `.set_context_node` to limit scope to this node
  ///
  /// Returns `Err` for a node without a document, such as `Node::null()` or a node of a dropped document.
  pub fn from_node(node: &Node) -> Result<Context, ()> {
    let docref = node.upgrade_docref().ok_or(())?;
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

  /// Shared body of the `*_checked` evaluators: reset the thread's last error
  /// (so [`XPathError::from_last_error`] reads THIS call's failure, not a stale
  /// one), run `eval`, and wrap the raw object — surfacing the structured error
  /// when libxml2 returns NULL. `eval` receives the NUL-terminated expression.
  fn eval_checked(
    &self,
    xpath: &str,
    eval: impl FnOnce(*const u8) -> xmlXPathObjectPtr,
  ) -> Result<Object, XPathError> {
    let c_xpath = CString::new(xpath).unwrap();
    unsafe { xmlResetLastError() };
    let ptr = eval(c_xpath.as_bytes().as_ptr());
    if ptr.is_null() {
      Err(XPathError::from_last_error())
    } else {
      Ok(self.object(ptr))
    }
  }

  ///evaluate an xpath
  pub fn evaluate(&self, xpath: &str) -> Result<Object, ()> {
    self.evaluate_checked(xpath).map_err(|_| ())
  }

  /// Evaluate `xpath`, returning libxml2's structured [`XPathError`] on failure
  /// instead of the bare `()` that [`Context::evaluate`] yields.
  pub fn evaluate_checked(&self, xpath: &str) -> Result<Object, XPathError> {
    self.evaluate_unwatched(xpath).map(Object::watched)
  }

  /// [`evaluate_checked`](Context::evaluate_checked), for a caller that reads the result before any node can be freed.
  fn evaluate_unwatched(&self, xpath: &str) -> Result<Object, XPathError> {
    self.forget_freed_context_node();
    self.eval_checked(xpath, |s| unsafe {
      xmlXPathEvalExpression(s, self.as_ptr())
    })
  }

  ///evaluate an xpath on a context Node
  pub fn node_evaluate(&self, xpath: &str, node: &Node) -> Result<Object, ()> {
    self.node_evaluate_checked(xpath, node).map_err(|_| ())
  }

  /// Evaluate `xpath` relative to `node`. See [`Context::evaluate_checked`].
  pub fn node_evaluate_checked(&self, xpath: &str, node: &Node) -> Result<Object, XPathError> {
    self
      .node_evaluate_unwatched(xpath, node)
      .map(Object::watched)
  }

  /// [`node_evaluate_checked`](Context::node_evaluate_checked), for a caller that reads the result before any node can
  /// be freed.
  fn node_evaluate_unwatched(&self, xpath: &str, node: &Node) -> Result<Object, XPathError> {
    let result = self.eval_checked(xpath, |s| unsafe {
      xmlXPathNodeEval(node.node_ptr(), s, self.as_ptr())
    });
    self.keep_context_node(node);
    result
  }

  ///evaluate an xpath on a context RoNode
  pub fn node_evaluate_readonly(&self, xpath: &str, node: RoNode) -> Result<Object, ()> {
    self
      .node_evaluate_readonly_checked(xpath, node)
      .map_err(|_| ())
  }

  /// Evaluate `xpath` relative to a read-only `node`. See
  /// [`Context::evaluate_checked`].
  pub fn node_evaluate_readonly_checked(
    &self,
    xpath: &str,
    node: RoNode,
  ) -> Result<Object, XPathError> {
    let result = self.eval_checked(xpath, |s| unsafe {
      xmlXPathNodeEval(node.0, s, self.as_ptr())
    });
    self.keep_readonly_context_node(node);
    result.map(Object::watched)
  }

  /// localize xpath context to a specific Node
  pub fn set_context_node(&mut self, node: &Node) -> Result<(), ()> {
    if unsafe { xmlXPathSetContextNode(node.node_ptr(), self.as_ptr()) } != 0 {
      return Err(());
    }
    self.keep_context_node(node);
    Ok(())
  }

  /// find nodes via xpath, at a specified node or the document root
  pub fn findnodes(&mut self, xpath: &str, node_opt: Option<&Node>) -> Result<Vec<Node>, ()> {
    let evaluated = match node_opt {
      Some(node) => self.node_evaluate_unwatched(xpath, node),
      None => self.evaluate_unwatched(xpath),
    }
    .map_err(|_| ())?;
    Ok(evaluated.get_nodes_as_vec())
  }

  /// find literal values via xpath, at a specified node or the document root
  pub fn findvalues(&mut self, xpath: &str, node_opt: Option<&Node>) -> Result<Vec<String>, ()> {
    let evaluated = match node_opt {
      Some(node) => self.node_evaluate_unwatched(xpath, node),
      None => self.evaluate_unwatched(xpath),
    }
    .map_err(|_| ())?;
    Ok(evaluated.get_nodes_as_str())
  }

  /// find a literal value via xpath, at a specified node or the document root
  pub fn findvalue(&mut self, xpath: &str, node_opt: Option<&Node>) -> Result<String, ()> {
    let evaluated = match node_opt {
      Some(node) => self.node_evaluate_unwatched(xpath, node),
      None => self.evaluate_unwatched(xpath),
    }
    .map_err(|_| ())?;
    Ok(evaluated.to_string())
  }

  /// Shared body of the compiled evaluators, as [`Context::eval_checked`] is the string evaluators': reset the
  /// thread's last error, run the expression, and wrap the raw object or surface the structured error.
  fn compiled_eval_checked(&self, expression: &Expression) -> Result<Object, XPathError> {
    unsafe { xmlResetLastError() };
    let ptr = unsafe { xmlXPathCompiledEval(expression.ptr, self.as_ptr()) };
    if ptr.is_null() {
      Err(XPathError::from_last_error())
    } else {
      Ok(self.object(ptr))
    }
  }

  /// Evaluate a compiled expression at the context's current node, as [`Context::evaluate_checked`] evaluates a
  /// string.
  pub fn evaluate_compiled(&self, expression: &Expression) -> Result<Object, XPathError> {
    self
      .evaluate_compiled_unwatched(expression)
      .map(Object::watched)
  }

  /// [`evaluate_compiled`](Context::evaluate_compiled), for a caller that reads the result before any node can be
  /// freed.
  fn evaluate_compiled_unwatched(&self, expression: &Expression) -> Result<Object, XPathError> {
    self.forget_freed_context_node();
    self.compiled_eval_checked(expression)
  }

  /// Evaluate a compiled expression relative to `node`, as [`Context::node_evaluate_checked`] evaluates a string:
  /// libxml2's `xmlXPathNodeEval` sets the context node, then runs the expression. Like it, this leaves `node` as the
  /// context's current node.
  pub fn node_evaluate_compiled(
    &self,
    expression: &Expression,
    node: &Node,
  ) -> Result<Object, XPathError> {
    self
      .node_evaluate_compiled_unwatched(expression, node)
      .map(Object::watched)
  }

  /// [`node_evaluate_compiled`](Context::node_evaluate_compiled), for a caller that reads the result before any node
  /// can be freed.
  fn node_evaluate_compiled_unwatched(
    &self,
    expression: &Expression,
    node: &Node,
  ) -> Result<Object, XPathError> {
    unsafe { xmlResetLastError() };
    if unsafe { xmlXPathSetContextNode(node.node_ptr(), self.as_ptr()) } < 0 {
      return Err(XPathError::from_last_error());
    }
    self.keep_context_node(node);
    self.compiled_eval_checked(expression)
  }

  /// find nodes via a compiled expression, at a specified node or the context's current node (see
  /// [`Context::findnodes`])
  pub fn findnodes_compiled(
    &self,
    expression: &Expression,
    node_opt: Option<&Node>,
  ) -> Result<Vec<Node>, XPathError> {
    let evaluated = match node_opt {
      Some(node) => self.node_evaluate_compiled_unwatched(expression, node)?,
      None => self.evaluate_compiled_unwatched(expression)?,
    };
    Ok(evaluated.get_nodes_as_vec())
  }

  /// find literal values via a compiled expression, at a specified node or the context's current node (see
  /// [`Context::findvalues`])
  pub fn findvalues_compiled(
    &self,
    expression: &Expression,
    node_opt: Option<&Node>,
  ) -> Result<Vec<String>, XPathError> {
    let evaluated = match node_opt {
      Some(node) => self.node_evaluate_compiled_unwatched(expression, node)?,
      None => self.evaluate_compiled_unwatched(expression)?,
    };
    Ok(evaluated.get_nodes_as_str())
  }

  /// find a literal value via a compiled expression, at a specified node or the context's current node (see
  /// [`Context::findvalue`])
  pub fn findvalue_compiled(
    &self,
    expression: &Expression,
    node_opt: Option<&Node>,
  ) -> Result<String, XPathError> {
    let evaluated = match node_opt {
      Some(node) => self.node_evaluate_compiled_unwatched(expression, node)?,
      None => self.evaluate_compiled_unwatched(expression)?,
    };
    Ok(evaluated.to_string())
  }
}

impl Drop for Object {
  /// free the memory allocated
  fn drop(&mut self) {
    if self.frees_seen.get().is_some() {
      self.unwatch();
    }
    unsafe {
      xmlXPathFreeObject(self.ptr);
    }
  }
}

impl Object {
  /// The `Object` for `ptr`, of an evaluation against `document`, not yet [`watched`](Self::watched).
  fn unwatched(ptr: xmlXPathObjectPtr, document: DocumentRef) -> Object {
    Object {
      ptr,
      document,
      frees_seen: Cell::new(None),
    }
  }

  /// This object, whose node set, if not empty, has its document log the nodes it frees from now on, so that it leaves
  /// them out (see [`leave_out_freed`](Self::leave_out_freed)): for an object handed out, which outlives frees.
  fn watched(self) -> Object {
    // SAFETY: `ptr` is a live XPath object, and its node set, if any, a live node set.
    let has_nodes = unsafe {
      let set = (*self.ptr).nodesetval;
      !set.is_null() && (*set).nodeNr > 0
    };
    if has_nodes {
      self
        .frees_seen
        .set(Some(self.document.borrow_mut().watch_node_set()));
    }
    self
  }

  /// Leaves the nodes its document freed since it last looked out of the node set, which libxml2 would read: also the
  /// namespace nodes of a freed element, copies the set owns whose `next` is the element (`xmlXPathFreeNodeSet` reads
  /// it), which it frees as libxml2 would.
  fn leave_out_freed(&self, doc: &mut _Document) {
    let Some(seen) = self.frees_seen.get() else {
      return;
    };
    if doc.frees_logged() == seen {
      return;
    }
    /// Up to this many frees, a linear search beats sorting a copy.
    const LINEAR: usize = 16;
    let freed = doc.freed_since(seen);
    let mut sorted = Vec::new();
    let freed = if freed.len() <= LINEAR {
      freed
    } else {
      sorted.extend_from_slice(freed);
      sorted.sort_unstable();
      &sorted[..]
    };
    let is_freed = |node: xmlNodePtr| {
      if freed.len() <= LINEAR {
        freed.contains(&node)
      } else {
        freed.binary_search(&node).is_ok()
      }
    };
    // SAFETY: the set is live (its object is). Every entry left is live: its node was live when the set was made, and
    // every free since was logged; or it is a namespace node the set owns.
    unsafe {
      let set = (*self.ptr).nodesetval;
      let count = (*set).nodeNr.max(0) as usize;
      let mut kept = 0;
      for i in 0..count {
        let node = *(*set).nodeTab.add(i);
        if is_freed(node) {
          continue;
        }
        if is_namespace(node) {
          let ns = node as xmlNsPtr;
          let element = (*ns).next as xmlNodePtr;
          if !element.is_null() && is_freed(element) {
            free_namespace_node(ns);
            continue;
          }
        }
        *(*set).nodeTab.add(kept) = node;
        kept += 1;
      }
      (*set).nodeNr = kept as c_int;
      if kept == 0 {
        // Empty for good: it no longer needs the log.
        self.frees_seen.set(None);
        doc.unwatch_node_set();
        return;
      }
    }
    self.frees_seen.set(Some(doc.frees_seen()));
  }

  /// Before the node set is freed, which reads every entry: [`leave_out_freed`](Self::leave_out_freed), and ends the
  /// document's [`watch_node_set`](_Document::watch_node_set).
  #[inline(never)]
  fn unwatch(&self) {
    match self.document.try_borrow_mut() {
      Ok(mut doc) => {
        self.leave_out_freed(&mut doc);
        // `leave_out_freed` ends the watch itself if the set is now empty.
        if self.frees_seen.take().is_some() {
          doc.unwatch_node_set();
        }
      }
      // Never the case: no method holds the document borrowed while an `Object` drops. Then reading no entry is safe;
      // it leaks the namespace nodes of the set, if any; the document logs frees until it drops.
      Err(_) => {
        unsafe { (*(*self.ptr).nodesetval).nodeNr = 0 };
        unwatch_node_set_on_thread();
      }
    }
  }

  /// [`leave_out_freed`](Self::leave_out_freed), before a method reads the node set.
  fn refresh(&self) {
    if let Some(seen) = self.frees_seen.get()
      && self.document.borrow().frees_logged() != seen
    {
      self.leave_out_freed(&mut self.document.borrow_mut());
    }
  }

  ///get the number of nodes in the result set
  pub fn get_number_of_nodes(&self) -> usize {
    self.refresh();
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

  /// returns the result set as a vector of `Node` objects, without the namespace axis's entries (see `is_namespace`)
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
      if is_namespace(ptr) {
        continue;
      }
      let node = Node::wrap(ptr, &self.document);
      vec.push(node);
    }
    vec
  }

  /// returns the result set as a vector of `RoNode` objects, without the namespace axis's entries (see `is_namespace`)
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
      if is_namespace(ptr) {
        continue;
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

/// Whether a node-set entry is a namespace node (the namespace axis): an `xmlNs` copy the XPath object owns and frees,
/// shorter than an `xmlNode`, so neither a `Node` nor an `RoNode` may wrap it. `get_nodes_as_str` still reads it.
fn is_namespace(ptr: xmlNodePtr) -> bool {
  xmlGetNodeType(ptr) == xmlElementType_XML_NAMESPACE_DECL
}

/// Frees a namespace node of a node set, a copy of a declaration, as `xmlXPathNodeSetFreeNs` does, without reading its
/// element.
///
/// # Safety
/// `ns` is a namespace node a node set owns, and leaves out.
unsafe fn free_namespace_node(ns: xmlNsPtr) {
  unsafe {
    if !(*ns).href.is_null() {
      bindgenFree((*ns).href as *mut c_void);
    }
    if !(*ns).prefix.is_null() {
      bindgenFree((*ns).prefix as *mut c_void);
    }
  }
  bindgenFree(ns as *mut c_void);
}

impl fmt::Display for Object {
  /// use if the XPath used was meant to return a string, such as string(//foo/@attr)
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.refresh();
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

/// Calls the binding to <http://xmlsoft.org/html/libxml-xpath.html#xmlXPathCompile> and return true if
/// a non-null pointer is returned. The idea is to use this to validate an xpath independent of context.
/// Tests describing what this validates in tests/xpath_tests.rs
pub fn is_well_formed_xpath(xpath: &str) -> bool {
  crate::init_parser();
  let c_xpath = CString::new(xpath).unwrap();
  let xml_xpath_comp_expr_ptr = unsafe { xmlXPathCompile(c_xpath.as_bytes().as_ptr()) };
  if xml_xpath_comp_expr_ptr.is_null() {
    false
  } else {
    // A compiled expression owns its steps; a plain free would leak them.
    unsafe { xmlXPathFreeCompExpr(xml_xpath_comp_expr_ptr) };
    true
  }
}

#[cfg(test)]
mod watch_tests {
  use super::Context;
  use crate::parser::Parser;
  use crate::tree::document::node_sets_live;

  const XML: &str = "<r><a id='1'>x</a><a id='2'>y</a></r>";

  /// A context at an `RoNode` watches the document's frees instead of wrapping the node, and stops when it moves on
  /// to a `Node`, or drops.
  #[test]
  fn a_readonly_context_node_is_watched_not_wrapped() {
    let doc = Parser::default().parse_string(XML).unwrap();
    let root = doc.get_root_element().unwrap();
    let cached = doc.0.borrow().cached_node_count();
    let mut ctx = Context::new(&doc).unwrap();
    let first = root.as_readonly().get_first_child().unwrap();
    assert_eq!(
      ctx
        .node_evaluate_readonly("count(.)", first)
        .unwrap()
        .to_string(),
      "1"
    );
    let second = first.get_next_sibling().unwrap();
    assert_eq!(
      ctx
        .node_evaluate_readonly("string(@id)", second)
        .unwrap()
        .to_string(),
      "2"
    );
    assert_eq!(doc.0.borrow().cached_node_count(), cached);
    assert!(node_sets_live());
    ctx.set_context_node(&root).unwrap();
    assert!(!node_sets_live());
    ctx.node_evaluate_readonly("count(.)", first).unwrap();
    assert!(node_sets_live());
    drop(ctx);
    assert!(!node_sets_live());
  }

  /// The `find*` methods read their results before any node can be freed, so they watch nothing; an `Object` handed
  /// out watches its node set until it drops, or its last node is freed.
  #[test]
  fn only_a_handed_out_node_set_is_watched() {
    let doc = Parser::default().parse_string(XML).unwrap();
    let root = doc.get_root_element().unwrap();
    let mut ctx = Context::new(&doc).unwrap();
    assert_eq!(ctx.findnodes("//a", Some(&root)).unwrap().len(), 2);
    assert_eq!(ctx.findvalues("//a", None).unwrap(), ["x", "y"]);
    assert!(!node_sets_live());
    let numbers = ctx.evaluate("count(//a)").unwrap();
    assert!(!node_sets_live());
    let nodes = ctx.evaluate("//a").unwrap();
    assert!(node_sets_live());
    for node in nodes.get_nodes_as_vec() {
      node.free_subtree();
    }
    assert!(node_sets_live());
    assert_eq!(nodes.get_number_of_nodes(), 0);
    assert!(!node_sets_live());
    drop((nodes, numbers));
    assert!(!node_sets_live());
  }

  /// The drop of a set whose nodes were all freed, with nothing read in between, ends the watch once.
  #[test]
  fn a_set_emptied_by_frees_and_dropped_unwatches_once() {
    let doc = Parser::default().parse_string(XML).unwrap();
    let ctx = Context::new(&doc).unwrap();
    let kept = ctx.evaluate("/r").unwrap();
    let nodes = ctx.evaluate("//a").unwrap();
    for node in nodes.get_nodes_as_vec() {
      node.free_subtree();
    }
    drop(nodes);
    assert!(node_sets_live());
    drop(kept);
    assert!(!node_sets_live());
  }
}
