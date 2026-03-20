use crate::{readonly::RoNode, tree::Document};

/// A read-only libxml2 Document
#[derive(Clone)]
pub struct RoDocument(pub(crate) Document);

// SAFETY: we promise to only provide methods that need read-only access.
unsafe impl Sync for RoDocument {}
unsafe impl Send for RoDocument {}

impl RoDocument {
  /// Get the root element of the document (read-only)
  pub fn get_root_readonly(&self) -> Option<RoNode> {
    self.0.get_root_readonly()
  }
}
