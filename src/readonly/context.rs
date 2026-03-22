use crate::{
  readonly::{RoDocument, RoNode, RoObject},
  xpath::Context,
};

/// A read-only libxml2 Context
#[derive(Clone)]
pub struct RoContext(Context);

// SAFETY: we promise to only provide methods that need read-only access.
unsafe impl Sync for RoContext {}
unsafe impl Send for RoContext {}

impl RoContext {
  /// create a read-only xpath context for a document
  pub fn new(owner: &RoDocument) -> Result<Self, ()> {
    let context = Context::new(&owner.0)?;
    Ok(Self(context))
  }

  /// evaluate an xpath
  pub fn evaluate(&self, xpath: &str) -> Result<RoObject, ()> {
    self.0.evaluate(xpath).map(RoObject)
  }

  ///evaluate an xpath on a context Node
  pub fn node_evaluate(&self, xpath: &str, node: &RoNode) -> Result<RoObject, ()> {
    self.0.node_evaluate_readonly(xpath, *node).map(RoObject)
  }

  /// evaluate an xpath on a context RoNode
  pub fn node_evaluate_readonly(&self, xpath: &str, node: RoNode) -> Result<RoObject, ()> {
    self.0.node_evaluate_readonly(xpath, node).map(RoObject)
  }

  /// find nodes via xpath, at a specified node or the document root
  pub fn findnodes(&self, xpath: &str, node_opt: Option<&RoNode>) -> Result<Vec<RoNode>, ()> {
    // Note: we cannot implemented this as `self.0.findnodes(...)` because that
    // method takes `&mut self`.
    let evaluated = if let Some(node) = node_opt {
      self.node_evaluate(xpath, node)?
    } else {
      self.evaluate(xpath)?
    };
    Ok(evaluated.get_nodes_as_vec())
  }

  /// find literal values via xpath, at a specified node or the document root
  pub fn findvalues(&self, xpath: &str, node_opt: Option<&RoNode>) -> Result<Vec<String>, ()> {
    let evaluated = if let Some(node) = node_opt {
      self.node_evaluate(xpath, node)?
    } else {
      self.evaluate(xpath)?
    };
    Ok(evaluated.get_nodes_as_str())
  }

  /// find a literal value via xpath, at a specified node or the document root
  pub fn findvalue(&self, xpath: &str, node_opt: Option<&RoNode>) -> Result<String, ()> {
    let evaluated = if let Some(node) = node_opt {
      self.node_evaluate(xpath, node)?
    } else {
      self.evaluate(xpath)?
    };
    Ok(evaluated.to_string())
  }
}
