[![Build Status](https://secure.travis-ci.org/KWARC/rust-libxml.png?branch=master)](http://travis-ci.org/KWARC/rust-libxml)
[![API Documentation](https://img.shields.io/badge/docs-API-blue.svg)](http://KWARC.github.io/rust-libxml/libxml/index.html)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/KWARC/rust-libxml/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/libxml.svg)](https://crates.io/crates/libxml)

Rust wrapper for [libxml2](http://xmlsoft.org/).

**CAUTION: Low-level wrapper without safety guarantees**

The main goal of this project is to benefit from libxml2's maturity and stability while the native Rust XML crates mature to be near-drop-in replacements.

As such, the crate exposes the libxml2 datastructures with minimal adaptations to the Rust model, leaving a lot of the potential concerns submerged in the C layer. That entails:

 * No Rust safety guarantees for mutability, as well as ownership - it is possible to own two Rust objects pointing to the same C pointer (with differing mutability).
 * No memory safety guarantees - it is possible to leak memory via e.g. careless use of unlink, among others.
 * No thread safety - libxml2's global memory management is a challenge to adapt in a thread-safe way with minimal intervention

**COVERAGE**: Only covers a subset of libxml2 at the moment, contributions are welcome. We try to increase support with each release.

**WELCOME!** With these caveats, the contributors to the project are migrating production work towards Rust and find a continuing reliance on libxml2 a helpful relief for initial ports. As such, contributions to this crate are welcome, if your workflow is not yet fully supported.
