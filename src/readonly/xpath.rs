use std::fmt;

use crate::{readonly::RoNode, xpath::Object};

/// Read-only version of the xpath object.
#[derive(Debug)]
pub struct RoObject(pub(crate) Object);

// SAFETY: we promise to only provide methods that need read-only access.
unsafe impl Sync for RoObject {}
unsafe impl Send for RoObject {}

impl RoObject {
  /// returns the result set as a vector of `RoNode` objects
  pub fn get_nodes_as_vec(&self) -> Vec<RoNode> {
    self.0.get_readonly_nodes_as_vec()
  }

  /// returns the result set as a vector of Strings
  pub fn get_nodes_as_str(&self) -> Vec<String> {
    self.0.get_nodes_as_str()
  }

  /// get the number of nodes in the result set
  pub fn get_number_of_nodes(&self) -> usize {
    self.0.get_number_of_nodes()
  }
}

impl fmt::Display for RoObject {
  /// use if the XPath used was meant to return a string, such as string(//foo/@attr)
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.0.fmt(f)
  }
}
