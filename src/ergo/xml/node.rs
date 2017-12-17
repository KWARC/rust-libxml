use libxml2::{ _xmlNode, xmlNodeSetName };
use std::ffi::{ CStr, CString };
use std::str;
use super::NodeSet;
/// An xml node
#[derive(Clone, Debug)]
pub struct Node {
    /// libxml's xmlNodePtr
    pub node_ptr: *mut _xmlNode,
}

impl Node {
    /// Returns all child nodes of the given node as a vector
    pub fn get_child_nodes(&self) -> NodeSet {
        let mut nodes = Vec::new();
        if let Some(node) = self.get_first_child() {
            nodes.push(node.clone());
            let mut current_node = node;
            while let Some(sibling) = current_node.get_next_sibling() {
                current_node = sibling.clone();
                nodes.push(sibling)
            }
        }
        NodeSet::new(nodes)
    }

    /// Returns the first child if it exists
    pub fn get_first_child(&self) -> Option<Node> {
        let ptr = unsafe { (*self.node_ptr).children };
        Node::ptr_as_option(ptr)
    }

    /// Returns the next sibling if it exists
    pub fn get_next_sibling(&self) -> Option<Node> {
        let ptr = unsafe { (*self.node_ptr).next };
        Node::ptr_as_option(ptr)
    }

    /// Returns the name of the node (empty string if name pointer is `NULL`)
    pub fn get_name(&self) -> String {
        let name_ptr = unsafe { (*self.node_ptr).name as *const i8 };
        if name_ptr.is_null() {
            return String::new();
        }  //empty string
        let c_string = unsafe { CStr::from_ptr(name_ptr) };
        str::from_utf8(c_string.to_bytes()).unwrap().to_owned()
    }

    /// Sets the name of this `Node`
    pub fn set_name(&mut self, name: &str) {
        let c_name = CString::new(name).unwrap();
        unsafe { xmlNodeSetName(self.node_ptr, c_name.as_ptr() as *const u8) }
    }

    fn ptr_as_option(node_ptr: *mut _xmlNode) -> Option<Node> {
        if node_ptr.is_null() {
            None
        } else {
            Some(Node {
                node_ptr,
            })
        }
    }
}
#[cfg(test)]
mod tests {
    use ergo::xml::{Document, Node};
    #[test]
    fn get_child_nodes_test(){
        let doc = Document::parse("<root><child1></child1><child2></child2></root>").unwrap();
        let mut children = doc.get_root_element().get_child_nodes();
        let children2 = doc.get_root_element().get_child_nodes();
        children.nodes()[0].set_name("lol");
        println!("{:?}", children2.nodes[0].get_name());

        /*
        let children2 = doc.get_root_element().get_child_nodes();
        let mut x = children2.nodes();
        x[0].set_name("lol");
        let y = children.nodes();
        println!("{}", y[0].get_name());
        */
    }
}
