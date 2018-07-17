//! The tree functionality
//!

pub mod document;
pub mod namespace;
pub mod node;

pub use tree::document::Document;
pub use tree::namespace::Namespace;
pub use tree::node::{Node, NodeType};

pub(crate) use tree::document::DocumentRef;
