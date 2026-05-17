# terafile

A Tree-sitter backed semantic parser for Tera template source.

`terafile` exposes a forgiving analysis API for editor and tooling workflows.
Invalid or incomplete template source is represented as structured diagnostics
instead of fatal errors whenever parsing can recover.

The crate currently vendors the Tree-sitter Tera grammar until upstream Rust
bindings are published to crates.io.
