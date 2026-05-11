//! Vendored Tera grammar binding.
//!
//! This mirrors the public surface of `uncenter/tree-sitter-tera` v0.1.0 so it
//! can be swapped for the crates.io package once upstream publishes it.

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_tera() -> *const ();
}

/// The tree-sitter language function for the vendored Tera grammar.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_tera) };

/// The content of the `node-types.json` file for this grammar.
pub const NODE_TYPES: &str = include_str!("../vendor/tree-sitter-tera/src/node-types.json");

/// The grammar's highlight query.
pub const HIGHLIGHTS_QUERY: &str =
    include_str!("../vendor/tree-sitter-tera/queries/highlights.scm");

/// The grammar's injection query.
pub const INJECTIONS_QUERY: &str =
    include_str!("../vendor/tree-sitter-tera/queries/injections.scm");

/// The grammar's locals query.
pub const LOCALS_QUERY: &str = include_str!("../vendor/tree-sitter-tera/queries/locals.scm");

#[cfg(test)]
mod tests {
    #[test]
    fn loads_vendored_tera_grammar() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("vendored Tera grammar should load");
    }

    #[test]
    fn parses_basic_tera_template() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("vendored Tera grammar should load");

        let tree = parser
            .parse("{% for item in items %}{{ item.name }}{% endfor %}", None)
            .expect("parser should produce a tree");

        assert!(!tree.root_node().has_error());
    }
}
