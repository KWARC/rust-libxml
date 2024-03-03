[![CI](https://github.com/KWARC/rust-libxml/actions/workflows/CI.yml/badge.svg?branch=master)](https://github.com/KWARC/rust-libxml/actions/workflows/CI.yml)
[![API Documentation](https://img.shields.io/badge/docs-API-blue.svg)](http://KWARC.github.io/rust-libxml/libxml/index.html)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/KWARC/rust-libxml/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/libxml.svg)](https://crates.io/crates/libxml)

Rust wrapper for [libxml2](http://xmlsoft.org/).

The main goal of this project is to benefit from libxml2's maturity and stability while the native Rust XML crates mature to be near-drop-in replacements.

As of the `0.2.0` release of the crate, there are some modest safety guarantees:

 * Mutability, as well as ownership - we use `Rc<RefCell<T>>` wrappers to ensure runtime safety of libxml2 operations already in the Rust layer.
 * Memory safety guarantees - in particular `Node` and `Document` objects have automatic bookkeeping and deallocation on drop, for leak-free wrapper use.
 * No thread safety - libxml2's global memory management is a challenge to adapt in a thread-safe way with minimal intervention

**Coverage**: Only covers a subset of libxml2 at the moment, contributions are welcome. We try to increase support with each release.

**Welcome!** With these caveats, the contributors to the project are migrating production work towards Rust and find a continuing reliance on libxml2 a helpful relief for initial ports. As such, contributions to this crate are welcome, if your workflow is not yet fully supported.

## Installation prerequisites

Before performing the usual cargo build/install steps, you need to have the relevant components for using the original libxml2 code. These may become gradually outdated with time - please do let us know by opening a new issue/PR whenever that's the case.

### Linux/Debian

On linux systems you'd need the development headers of libxml2 (e.g. `libxml2-dev` in Debian), as well as `pkg-config`.

### MacOS

With the ability of [custom cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html), we can now override build scripts per project separately without the need of modifying environment variables.

Firstly, install relevant librarys by [homebrew](https://brew.sh/)

```
$ brew install libxml2 # e.g. version 2.9.13 
```

Then we can manually [override](https://doc.rust-lang.org/cargo/reference/config.html#targettriplelinks) the path of native library in a **project-level** configuration separately.

* make sure we are in the same folder where `Cargo.toml` located in.
* create a dir named `.cargo` with a file named `config.toml` in it. 
* Then add our custom build configuration like below. Also, don't forget to change the target and library paths to yours.

  
```
$ cd /path/to/your/project
$ mkdir .cargo
$ vim .cargo/config.toml
$ cat .cargo/config.toml
[target.YOUR_TARGET.libxml]
rustc-link-lib = ["xml2"]
rustc-link-search = ["/path/to/lib"]
```

Targets may **different** between the archs and operating systems you are using. To figure out which you should use, simply run `rustup target list`, then replace `YOUR_TARGET` by the name of the installed one.

```
$ rustup target list
aarch64-apple-darwin (installed)
aarch64-apple-ios
aarch64-apple-ios-sim
aarch64-fuchsia
aarch64-linux-android
...
...
...

# so we edit config.toml like below:
$ cat .cargo/config.toml
[target.aarch64-apple-darwin.libxml]
rustc-link-lib = ["xml2"]
rustc-link-search = ["/opt/homebrew/opt/libxml2/lib/"]
```


`rustc-link-search` indicates the path where compiler would try to find native library in. It should be set according to your own environment.

### Windows

[Community contributed](https://github.com/KWARC/rust-libxml/issues/81#issuecomment-760364976):

* manually install builds tools c++ and english language by visiting [BuildTools](https://visualstudio.microsoft.com/fr/thank-you-downloading-visual-studio/?sku=BuildTools&rel=16)
* launch cmd prompt with admin privileges and execute these commands sequentially:
```
C:\> git clone https://github.com/microsoft/vcpkg
C:\> .\vcpkg\bootstrap-vcpkg.bat
C:\> setx /M PATH "%PATH%;c:\vcpkg" && setx VCPKGRS_DYNAMIC "1" /M
C:\> refreshenv
C:\> vcpkg install libxml2:x64-windows
C:\> vcpkg integrate install
```

If you encounter any errors in the build script(`build.rs`), just do the same steps to manually set the path in section `MacOS` above 