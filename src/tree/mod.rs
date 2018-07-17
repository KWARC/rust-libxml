//! The tree functionality
//!

pub mod document;
pub mod namespace;
pub mod node;
pub mod nodetype;

pub use tree::document::Document;
pub(crate) use tree::document::DocumentRef;
pub use tree::namespace::Namespace;
pub use tree::node::Node;
pub use tree::nodetype::NodeType;
