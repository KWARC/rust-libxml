# Change Log

## [0.2.15] (in active development)

## [0.2.14] 2020-27-03

### Changed

More consistently use `c_char` to successfully compile on ARM targets

## [0.2.13] 2020-16-01

Thanks to @jangernert for the upgrades to `Document` serialization.
Thanks to @lweberk for contributing the `Schema` featureset and to @cbarber for refining the FFI interop.

### Added
 * `Document::to_string_with_options` allowing to customize document serialization
 * `Document::SaveOptions` containing the currently supported serialization options, as provided internally by libxml
 * `Schema` holding and managing `xmlSchemaPtr` as created while parsing by `SchemaParserContext`
 * `SchemaParserContext` holding source of XSD and parsing into a `Schema` while gathering and –in case returning– errors that arise from the XSD parser across the FFI to libxml
 * `SchemaValidationContext` holding the `Schema` from resulting `SchemaParserContext` parse and offering validation methods for `Document`, `Node` or file path to XML, while gathering and –in case returning– validation errors from the XML validator across the FFI to libxml

### Changed
 * the `Document::to_string()` serialization method is now implemented through `fmt::Display` and no longer takes an optional boolean flag. The default behavior is now unformatted serialization - previously `to_string(false)`, while `to_string(true)` can be realized via
 ```
   .to_string_with_options(SaveOptions { format: true, ..SaveOptions::default()})`
  ```

## [0.2.12] 2019-16-06

Thanks to @Alexhuszagh for contributing all enhancements for the `0.2.12` release!

### Added

  * BOM-aware Unicode support
  * New `Parser` methods allowing to specify an explicit encoding: `parse_file_with_encoding`, `parse_string_with_encoding`, `is_well_formed_html_with_encoding`

### Changed

  * Default encodings in `Parser` are now left for libxml to guess internally, rather than defaulted to `utf-8`.

## [0.2.11] 2019-15-04

### Added
  * `RoNode::to_hashable` and `RoNode::null` for parity with existing `Node`-leveraging applications

## [0.2.10] 2019-14-04

### Added

 * `RoNode` primitive for simple and efficient **read-only** parallel processing
 * Benchmarking a 120 MB XML document shows a twenty five fold speedup, when comparing `Node` to parallel rayon processing over `RoNode` with a 32 logical core desktop
 * While `RoNode` is added as an experiment for high performance read-only scans, any mutability requires using `Node` and incurring a bookkeeping cost of safety at runtime.
 * Introduced benchmarking via `criterion`, only installed during development.
 * `benches/parsing_benchmarks` contains examples of parallel scanning via `rayon` iterators.
 * added `Document::get_root_readonly` method for obtaining a `RoNode` root.
 * added `Context::node_evaluate_readonly` method for searching over a `RoNode`
 * added `Context::get_readonly_nodes_as_vec` method for collecting xpath results as `RoNode`

## [0.2.9] 2019-28-03

  * Squash memory leak in creating new `Node`s from the Rust API
  * Safely unlink `Node`s obtained via XPath searches

## [0.2.8] 2019-25-03

### Changed

Minor internal changes to make the crate compile more reliably under MacOS, and other platforms which enable the `LIBXML_THREAD_ENABLED` compile-time flag. Thank you @caldwell !

## [0.2.7] 2019-09-03

### Added

 * implement and test `replace_child_node` for element nodes

## [0.2.6] 2018-07-12

 * Internal update to Rust 2018 Edition
 * fix deallocation bugs with `.import_node()` and `.get_namespaces()`

## [0.2.5] 2018-26-09

### Added
 * `Node::null` placeholder that avoids the tricky memory management of `Node::mock` that can lead to memory leaks. Really a poor substitute for the better `Option<Node>` type with a `None` value, which is **recommended** instead.

## [0.2.4] 2018-24-09

### Added
 * `Context::from_node` method for convenient XPath context initialization via a Node object. Possible as nodes keep a reference to their owner `Document` object.

 ### Changed
  * Ensured memory safety of cloning xpath `Context` objects
  * Switched to using `Weak` references to the owner document, in `Node`, `Context` and `Object`, to prevent memory leaks in mutli-document pipelines.
  * Speedup to XPath node retrieval

## [0.2.3] 2018-19-09

### Added
 * `Node::findnodes` method for direct XPath search, without first explicitly instantiating a `Context`. Reusing a `Context` remains more efficient.

## [0.2.2] 2018-23-07

 * Expose the underlying `libxml2` data structures in the public crate interface, to enable a first [libxslt](https://crates.io/crates/libxslt) crate proof of concept.

## [0.2.1] 2018-23-07

### Added

 * `Node::set_node_rc_guard` which allows customizing the reference-count mutability threshold for Nodes.
 * serialization tests for `Document`
 * (crate internal) full set of libxml2 bindings as produced via `bindgen` (see #39)
 * (crate internal) using libxml2's type language in the wrapper Rust modules
 * (crate internal) setup bindings for reuse in higher-level crates, such as libxslt


### Changed

 * `NodeType::from_c_int` renamed to `NodeType::from_int`, now accepting a `u32` argument

### Removed

 * Removed dependence on custom C code; also removed gcc from build dependencies


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
