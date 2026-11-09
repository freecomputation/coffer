# Umiak

by Free Computation. AGPL-3.0.

A browser for people with unstable or no internet. Umiak is fully
functional with no connection at all: its internet is the Shelf, a
library aboard the machine - Wikipedia and thousands of other open
corpora in ZIM form, maps, books, documentation. When a connection
appears it drinks: verified content arrives in chunks, from the network
or from a stick passed hand to hand, and is checked before it is
shelved.

Umiak is the first form of idem, the system that grows underneath it.

## Layout

- `crates/zim-ffi` - the only C++ in the project: a thin C shim over
  libzim with a safe Rust wrapper.
- `crates/umiak-core` - the Shelf: discovery and reading of the books
  aboard.
- `crates/umiak-server` - the loopback HTTP server every surface talks
  to. Shelf home, book content, one search across everything aboard.
- `app/` - the Tauri shell: one system-webview window onto the server.
  Deliberately thin; the engine is borrowed and swappable.

## Build and run

Needs a Rust toolchain and libzim (runtime plus headers).

With a normal libzim development install:

    cargo build
    UMIAK_LIBZIM_LIB=$(ldconfig -p | grep -m1 'libzim.so' | awk '{print $NF}') \
      cargo run -p umiak-server -- --shelf /path/to/zims

Where only the runtime is installed, extract the headers from the
libzim-dev package into `third-party/libzim-dev/` (the build finds
them there; the directory is git-ignored) and point
`UMIAK_LIBZIM_LIB` at the runtime library, for example
`/lib/x86_64-linux-gnu/libzim.so.8`:

    apt-get download libzim-dev
    dpkg -x libzim-dev_*.deb third-party/libzim-dev

Then open http://127.0.0.1:27331/ - the Shelf home.

## Design

See DESIGN.md for the determinations this code answers to.
