use super::Node;

#[derive(Debug)]
pub struct NodeSet {
    pub nodes: Vec<Node>
}

impl NodeSet {
    pub fn new(nodes: Vec<Node>) -> Self {
        NodeSet{nodes}
    }
    pub fn nodes(self) -> Vec<Node> {
       self.nodes
    }
}