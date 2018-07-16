[![Build Status](https://secure.travis-ci.org/KWARC/rust-libxml.png?branch=master)](http://travis-ci.org/KWARC/rust-libxml)
[![API Documentation](https://img.shields.io/badge/docs-API-blue.svg)](http://KWARC.github.io/rust-libxml/libxml/index.html)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/KWARC/rust-libxml/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/libxml.svg)](https://crates.io/crates/libxml)

Rust wrapper for [libxml2](http://xmlsoft.org/).

The main goal of this project is to benefit from libxml2's maturity and stability while the native Rust XML crates mature to be near-drop-in replacements.

As of the (upcoming) `0.2.0` release of the crate, there are some modest safety guarantees:

 * Mutability, as well as ownership - we use `Rc<RefCell<T>>` wrappers to ensure runtime safety of libxml2 operations already in the Rust layer.
 * Memory safety guarantees - in particular `Node` and `Document` objects have automatic bookkeeping and deallocation on drop, for leak-free wrapper use.
 * No thread safety - libxml2's global memory management is a challenge to adapt in a thread-safe way with minimal intervention

**Coverage**: Only covers a subset of libxml2 at the moment, contributions are welcome. We try to increase support with each release.

**Welcome!** With these caveats, the contributors to the project are migrating production work towards Rust and find a continuing reliance on libxml2 a helpful relief for initial ports. As such, contributions to this crate are welcome, if your workflow is not yet fully supported.
