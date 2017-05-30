# Change Log

## [0.0.75 (in active dev)]

### Added

* Node methods: `get_namespace_declarations`, `get_property_ns` (alias: `get_attribute_ns`), `remove_property` (alias: `remove_attribute`), `get_attribute_node`, `get_namespace`, `lookup_namespace_prefix`, `lookup_namespace_uri`

* XPath methods: `findvalue` and `findnodes`, with optional node-bound evaluation.

### Changed

* The Node setter for a namespaced attribute is now `set_property_ns` (alias: `set_attribute_ns`)
* Node set_* methods are now consistently defined on `&mut self`
* Refactored wrongly used `url` to `href` for namespace-related Node ops.
* Fixed bug with Node's `get_content` method always returning empty

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
