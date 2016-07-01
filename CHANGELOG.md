# Change Log

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
