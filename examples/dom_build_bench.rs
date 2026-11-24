//! Builds a large document the way a converter does (latexml-oxide's `Document`), to measure the per-node cost of the
//! tree-editing API: run under `perf stat -e instructions:u`. Usage: `dom_build_bench [elements] [chain depth] [seed]`.
//!
//! Every step creates nodes and links them at the insertion point, with attributes, appends text, and now and then
//! moves a finished subtree under a new wrapper element, unlinks and frees a temporary node, or queries by XPath.
//! Then a chain of nested groups, each holding a few drawn elements, is collapsed from the innermost out, each
//! group's children moved after it (`add_next_sibling`) and the group removed, as latexml-oxide collapses the
//! `svg:g` elements a TikZ picture nests (arXiv 2605.19500).
use libxml::parser::Parser;
use libxml::tree::{Document, Node};
use libxml::xpath::Context;

/// A small, fixed pseudo-random sequence (an LCG), so that every run builds the same tree.
struct Lcg(u64);
impl Lcg {
  fn next(&mut self, bound: u64) -> u64 {
    self.0 = self
      .0
      .wrapping_mul(6364136223846793005)
      .wrapping_add(1442695040888963407);
    (self.0 >> 33) % bound
  }
}

fn main() {
  let mut args = std::env::args().skip(1);
  let elements: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(100_000);
  let chain_depth: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(3_000);
  let mut rng = Lcg(args.next().and_then(|a| a.parse().ok()).unwrap_or(1));
  let parser = Parser::default();
  let doc: Document = parser
    .parse_string("<document xmlns='http://dlmf.nist.gov/LaTeXML'/>")
    .unwrap();
  let root = doc.get_root_element().unwrap();
  // The open elements, innermost last, as a converter's insertion point.
  let mut stack: Vec<Node> = vec![root.clone()];
  let names = [
    "para", "p", "text", "Math", "XMath", "XMApp", "XMTok", "g", "picture", "note",
  ];
  let mut made = 0;
  let mut kept = Vec::new();
  while made < elements {
    let action = rng.next(100);
    let mut point = stack.last().unwrap().clone();
    if action < 45 || stack.len() < 3 {
      // Open an element with attributes at the point, and descend into it now and then.
      let name = names[rng.next(names.len() as u64) as usize];
      let mut node = Node::new(name, None, &doc).unwrap();
      point.add_child(&mut node).unwrap();
      node.set_attribute("xml:id", &format!("S{made}")).unwrap();
      node.set_attribute("class", "ltx_p").unwrap();
      if rng.next(2) == 0 {
        node.set_attribute("font", "italic").unwrap();
      }
      node.set_attribute("tex", "x").unwrap();
      made += 1;
      if rng.next(3) == 0 && stack.len() < 40 {
        stack.push(node.clone());
      }
      if made.is_multiple_of(97) {
        kept.push(node);
      }
    } else if action < 70 {
      // Text: a new text node, then more text appended to it.
      let mut text = Node::new_text("some text", &doc).unwrap();
      point.add_child(&mut text).unwrap();
      point.append_text(" more").unwrap();
    } else if action < 80 {
      // Close the innermost element.
      if stack.len() > 1 {
        stack.pop();
      }
    } else if action < 88 {
      // Move the point's last child, a finished subtree, under a new wrapper element (latexml's `wrapNodes`).
      if let Some(mut last) = point.get_last_child()
        && last.is_element_node()
      {
        let mut wrapper = Node::new("inline-block", None, &doc).unwrap();
        wrapper.set_attribute("class", "ltx_wrap").unwrap();
        last.unlink_node();
        point.add_child(&mut wrapper).unwrap();
        wrapper.add_child(&mut last).unwrap();
        made += 1;
      }
    } else if action < 93 {
      // A temporary node, inserted before the point's first child and removed again.
      if let Some(mut first) = point.get_first_child() {
        let mut temp = Node::new("ERROR", None, &doc).unwrap();
        first.add_prev_sibling(&mut temp).unwrap();
        temp.set_attribute("class", "undefined").unwrap();
        temp.unlink_node();
      }
    } else if action < 98 {
      // Read the neighbourhood: the parent chain and the children.
      let mut node = point.clone();
      while let Some(parent) = node.get_parent() {
        node = parent;
      }
      let _ = point.get_child_nodes().len();
      let _ = point.get_attribute("class");
    } else {
      // Set the content of the point's last text child.
      if let Some(mut last) = point.get_last_child()
        && last.is_text_node()
      {
        last.set_content("replaced").unwrap();
      }
    }
  }
  // The chain of nested groups, under the last element made, and its collapse from the innermost group out.
  let mut groups = Vec::with_capacity(chain_depth);
  let mut point = root.clone();
  for depth in 0..chain_depth {
    let mut group = Node::new("g", None, &doc).unwrap();
    point.add_child(&mut group).unwrap();
    group.set_attribute("transform", "translate(1,1)").unwrap();
    for leaf in 0..3 {
      let mut path = Node::new("path", None, &doc).unwrap();
      group.add_child(&mut path).unwrap();
      path
        .set_attribute("d", &format!("M {depth} {leaf} L 1 1"))
        .unwrap();
      path.set_attribute("stroke", "black").unwrap();
    }
    groups.push(group.clone());
    point = group;
  }
  for mut group in groups.into_iter().rev() {
    let mut anchor = group.clone();
    for mut child in group.get_child_nodes() {
      child.unlink_node();
      anchor.add_next_sibling(&mut child).unwrap();
      anchor = child;
    }
    group.unlink_node();
  }
  // A few queries over the whole document, as the post-processing of a converter makes.
  let mut context = Context::new(&doc).unwrap();
  let mut found = 0;
  for _ in 0..5 {
    found += context.findnodes("//*[@font]", None).unwrap().len();
  }
  for node in &kept {
    found += node.findnodes("ancestor::*").unwrap().len();
  }
  println!(
    "{made} elements, a chain of {chain_depth} groups, {found} found, {} bytes",
    doc.to_string().len()
  );
}
