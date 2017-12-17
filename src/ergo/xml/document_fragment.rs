use super::{ Document, NodeSet, XmlError };
pub struct DocumentFragment {

}

impl DocumentFragment {
    pub fn new(xml_str: &str) -> Result<NodeSet, Vec<XmlError>> {
        match Document::parse(format!("<root>{}</root>", xml_str).as_str()) {
            Ok(doc) => {
                Ok(doc.get_root_element().get_child_nodes())
            },
            Err(err) => Err(err)
        }
    }
}