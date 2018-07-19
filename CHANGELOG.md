# Change Log
## [0.2.1 (in active dev)]
 * Added `Node::set_node_rc_guard` which allows customizing the reference-count mutability threshold for Nodes.

## [0.2.0] 2018-19-07

This release adds fundamental breaking changes to the API. The API continues to be considered unstable until the `1.0.0` release.

### Added
 * `dup` and `dup_from` methods for deeply duplicating a libxml2 document
 * `is_unlinked` for quick check if a `Node` has been unlinked from a parent

### Changed
 * safe API for `Node`s and `Document`s, with automatic pointer bookkeeping and memory deallocation, by @triptec
   * `Node`s are now bookkept by their owning document
   * libxml2 low-level memory deallocation is postponed until the `Document` is dropped, with the exception of unlinked nodes, who are deallocated on drop.
   * `Document::get_root_element` now has an option type, and returns `None` for an empty Document
   * `Node::mock` now takes owner `Document` as argument
   * proofed tests with `valgrind` and removed all obvious memory leaks
 * All node operations that modify a `Node` now both require a `&mut Node` argument and return a `Result` type. 
   * Full list of changed signatures in Node: `remove_attribute`, `remove_property`, `set_name`, `set_content`, `set_property`, `set_property_ns`, `set_attribute`, `set_attribute_ns`, `remove_attribute`, `set_namespace`, `recursively_remove_namespaces`, `append_text` 
 * Tree transforming operations that use operate on `&mut self`, and no longer return a Node if the return value is identical to the argument. 
   * Changed signatures: `add_child`, `add_prev_sibling`, `add_next_sibling`
 * `Result` types should always be checked for errors, as mutability conflicts are reported during runtime.
### Removed
 * `global` module, which attempted to manage global libxml state for threaded workflows. May be readed after the API stabilizes


## [0.1.2] 2018-12-01

* We welcome Andreas (@triptec) to the core developer team!

### Added
 
* Workaround `.free` method for freeing nodes, until the `Rc<RefCell<_Node>>` free-on-drop solution by Andreas is introduced in 0.2

## [0.1.1] 2017-18-12

### Added

* `get_first_element_child` - similar to `get_first_child` but only returns XML Elements
* `is_element_node` - check if a given `Node` is an XML Element

### Changed

* Requiring owned `Node` function arguments only when consumed - `add_*` methods largely take `&Node` now.

## [0.1.0] 2017-09-11

Pushing up release to a 0.1, as contributor interest is starting to pick up, and the 0. version were getting a bit silly/wrong.

### Added

* Node methods: `unbind_node`,  `recursively_remove_namespaces`, `set_name`,  
* Document methods: `import_node`

### Changed

* Updated gcc build to newer incantation, upped dependency version.

## [0.0.75] 2017-04-06

### Added

* Node methods: `get_namespace_declarations`, `get_property_ns` (alias: `get_attribute_ns`), `remove_property` (alias: `remove_attribute`), `get_attribute_node`, `get_namespace`, `lookup_namespace_prefix`, `lookup_namespace_uri`

* XPath methods: `findvalue` and `findnodes`, with optional node-bound evaluation.

### Changed

* The Node setter for a namespaced attribute is now `set_property_ns` (alias: `set_attribute_ns`)
* Node set_* methods are now consistently defined on `&mut self`
* Refactored wrongly used `url` to `href` for namespace-related Node ops.
* Fixed bug with Node's `get_content` method always returning empty
* More stable `append_text` for node, added tests

## [0.0.74] 2016-25-12

### Changed

* Namespace::new only requires a borrowed &Node now
* Fixed bug with wrongly discarded namespace prefixes on Namespace::new

### Added

* Namespace methods: `get_prefix`, `get_url`


## [0.0.73] 2016-25-12

### Added

* Document method: `as_node`

## [0.0.72] 2016-25-12

### Added

* Node methods: `get_last_child`, `get_child_nodes`, `get_child_elements`, `get_properties`, `get_attributes`

## [0.0.71] 2016-29-11

### Changed

* Namespace::new takes Node argument last

### Added

* Node namespace accessors - `set_namespace`, `get_namespaces`, `set_ns_attribute`, `set_ns_property`
* Namespace registration for XPath

## [0.0.7] 2016-27-11

### Changed

* stricter dependency spec in Cargo.toml
* cargo clippy compliant
* Document's `get_root_element` returns the document pointer as a Node for empty documents, type change from `Option<Node>` to simple `<Node>`

### Added

* Node accessors: `set_attribute`, `get_attribute`, `set_property` (the `attribute` callers are simple aliases for `property`)
* Node `to_hashable` for simple hashing of nodes
* Node `mock` for simple mock nodes in testing


## [0.0.5] 2016-07-01

Thanks to @grray for most of these improvements!

### Changed

* Switched to using the more permissive MIT license, consistent with libxml2 licensing
* Fixed segfault issues with xpath contexts

### Added

* Can now evaluate ```string(/foo//@bar)``` type XPath expressions, and use their result via ```.to_string()```

## [0.0.4] 2016-04-25

### Changed

* The ```Node.add_child``` method now adds a Node, while the old behavior of creating a new node with a given namespace and name is now ```Node.new_child```

### Added

* Can add following siblings via ```Node.add_next_sibling```
* Can now add text nodes via ```Node.new_text```
