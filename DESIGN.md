# Umiak - design determinations

Determined 2026-11-09 by commissioned research over the whole design
space, against the idem charter's laws. The full sourced report is the
project record; this file is the working summary the code answers to.

Umiak ships first: a standalone browser, fully functional with no
internet, whose internet is the Shelf aboard. idem, the system, grows
underneath it later, and Umiak becomes its shell.

Engine: system webviews through Tauri 2 and Wry (WebView2, WKWebView, WebKitGTK, Android WebView), all content served over loopback HTTP. Fallback: CEF. Designated future swap: Servo. Ladybird is not ready.

Content: ZIM through libzim directly, behind one thin isolated FFI, served by our own Rust server following Kiwix URL and OPDS conventions. libkiwix is a prototype aid, never a dependency.

Search: Xapian inside each corpus through libzim's own searcher, untouched. Across the Shelf: fan-out plus rank fusion, with SQLite FTS5 for the catalogue and the user's own material. Tantivy is held for Umiak-native and grown content.

Language: Rust end to end. C++ exists only behind the FFI shim.

Growth (idem): the Nix derivation model, no existing implementation. Our own grower, derivations as data, small enough to read whole.

Base (idem): FreeBSD 15, ratified 2026-11-09 - Capsicum gives the broker its capability model natively, one coherent base tree, ZFS snapshots and boot environments, veriexec signed manifests. Bedrock only: replaceable, sealed, outside the trusted core, which is the seed, the grower, the verifiers, the capability broker, and the keys. Gated on a hardware census of cheap machines before the system phase; the Linux immutable image pattern is the standing fallback.

Grown code (idem): WebAssembly components on Wasmtime, WASI, deterministic profile with fuel. Nothing grown holds ambient authority.

Intelligence (both): an unprivileged local model service proposing through the capability broker. The owner's power structure is a machine, not a policy: the broker runs a two attempt state machine, first attempt returns a signed warning, second attempt requires the owner's confirmation to mint a one shot token bound to the action. The intelligence cannot mint tokens or confirm itself.

Sync: desync chunks over any transport, including a folder on a USB stick. Verification: Ed25519 signed edition manifests plus a witness cosigned transparency log whose proofs travel offline.


First benchmarks owed: desync chunk reuse across successive
recompressed ZIM editions, and search latency on genuinely weak
hardware. Neither has published numbers anywhere.
