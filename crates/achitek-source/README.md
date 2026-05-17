# achitek-source

Shared source-analysis primitives for Achitek crates.

This crate intentionally stays small. It owns common plumbing such as source
positions, source ranges, spanned values, diagnostic severity, and Tree-sitter
helper functions.

Language-specific models, parsers, and diagnostic codes belong in their
language crates. Those crates can re-export these primitives when doing so keeps
their public APIs convenient.
