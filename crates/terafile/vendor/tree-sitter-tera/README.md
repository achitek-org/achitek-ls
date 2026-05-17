# tree-sitter-tera vendor copy

This directory vendors the generated parser files from
`https://github.com/uncenter/tree-sitter-tera` so `achitek-ls` can continue to
publish to crates.io while the upstream Rust bindings are not yet available on
crates.io.

Vendored upstream version: `v0.1.0`
Vendored upstream commit: `44eb40ba80234cbb94194e358e90ebd3e7a6c918`

When upstream publishes `tree-sitter-tera` to crates.io, remove this directory,
remove the root `build.rs`, replace `crate::tree_sitter_tera` usages with the
published crate, and remove the `cc` build dependency.
