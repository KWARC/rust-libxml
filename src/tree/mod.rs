//! The tree functionality
//!

mod document;
mod namespace;
mod node;
mod nodetype;

pub use tree::document::Document;
pub(crate) use tree::document::DocumentRef;
pub use tree::namespace::Namespace;
pub use tree::node::Node;
pub use tree::nodetype::NodeType;
