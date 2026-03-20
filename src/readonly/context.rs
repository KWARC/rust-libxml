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

  /// evaluate an xpath on a context RoNode
  pub fn node_evaluate_readonly(&self, xpath: &str, node: RoNode) -> Result<RoObject, ()> {
    self.0.node_evaluate_readonly(xpath, node).map(RoObject)
  }
}
