//! The tree functionality
//!

pub mod document;
pub mod namespace;
pub mod node;
pub mod nodetype;

pub use self::document::Document;
pub(crate) use self::document::{DocumentRef, DocumentWeak};
pub use self::namespace::Namespace;
pub use self::node::set_node_rc_guard;
pub use self::node::{Node, NODE_RC_MAX_GUARD};
pub use self::nodetype::NodeType;
