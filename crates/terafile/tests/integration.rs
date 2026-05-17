#[cfg(test)]
mod parse_tree {
    use indoc::indoc;

    #[test]
    fn parses_file_and_returns_tree() {
        let source = indoc! {r#"
            {% for prompt in prompts %}
              {{ prompt.name }}
            {% endfor %}
        "#};

        let tree = terafile::parse(source).expect("valid Tera should parse");

        assert_eq!(tree.root_node().kind(), "source_file");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn parses_invalid_source_with_error_nodes() {
        let tree = terafile::parse("{% if enabled %}{{ name }}")
            .expect("tree-sitter should still produce a tree");

        assert!(tree.root_node().has_error());
    }
}
