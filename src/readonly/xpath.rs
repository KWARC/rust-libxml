use crate::{readonly::RoNode, xpath::Object};

/// Read-only version of the xpath object.
#[derive(Debug)]
pub struct RoObject(pub(crate) Object);

// SAFETY: we promise to only provide methods that need read-only access.
unsafe impl Sync for RoObject {}
unsafe impl Send for RoObject {}

impl RoObject {
  /// returns the result set as a vector of `RoNode` objects
  pub fn get_readonly_nodes_as_vec(&self) -> Vec<RoNode> {
    self.0.get_readonly_nodes_as_vec()
  }
}
