//! The tree functionality
//!

mod document;
mod namespace;
mod node;
mod nodetype;

pub use tree::document::Document;
pub(crate) use tree::document::{DocumentRef, DocumentWeak};
pub use tree::namespace::Namespace;
pub use tree::node::set_node_rc_guard;
pub use tree::node::{Node, NODE_RC_MAX_GUARD};
pub use tree::nodetype::NodeType;
