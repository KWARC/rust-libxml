//! A few random tests.
//! Knowing how much I neglect this aspect of software development,
//! there probably won't be a significant coverage.

#![feature(hash)]
extern crate rustlibxml;

use rustlibxml::tree::{XmlDoc, XmlNodeRef};
use std::hash::{hash, SipHasher};

#[test]
/// Duplicate an xml file
fn duplicate_file() {
    let doc = XmlDoc::parse_file("tests/resources/file01.xml").unwrap();
    doc.save_file("tests/resources/copy.xml").unwrap();
}

#[test]
/// Root node and first child of root node have different hash values.
/// (There is a tiny chance this might fail for a correct program)
fn child_of_root_has_different_hash() {
    let doc = XmlDoc::parse_file("tests/resources/file01.xml").unwrap();
    let root = doc.get_root_element().unwrap();
    if let Some(child) = root.get_first_child() {
        assert!((hash::<XmlNodeRef, SipHasher>(&root)) !=
                (hash::<XmlNodeRef, SipHasher>(&child)));
//        assert!((hash::<XmlNodeRef, SipHasher>(&root)) != hash(&child));
    } else {
        assert!(false);   //test failed - child doesn't exist
    }
}
