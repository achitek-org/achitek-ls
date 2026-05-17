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

#[cfg(test)]
mod diagnostics {
    use terafile::{DiagnosticCode, DiagnosticKind, Severity};

    #[test]
    fn exposes_stable_code_metadata() {
        let code = DiagnosticCode::UndefinedVariable;

        assert_eq!(code.as_str(), "TERA3003");
        assert_eq!(code.kind(), DiagnosticKind::Expression);
        assert_eq!(code.severity(), Severity::Error);
        assert_eq!(code.message(), "undefined variable");
        assert!(code.help().is_some());
    }

    #[test]
    fn reports_syntax_code_metadata() {
        let code = DiagnosticCode::SyntaxError;

        assert_eq!(code.as_str(), "TERA0000");
        assert_eq!(code.kind(), DiagnosticKind::Syntax);
        assert_eq!(code.severity(), Severity::Error);
        assert_eq!(code.message(), "syntax error");
    }

    #[test]
    fn reports_dependency_code_metadata() {
        let code = DiagnosticCode::DynamicIncludePath;

        assert_eq!(code.as_str(), "TERA2001");
        assert_eq!(code.kind(), DiagnosticKind::Dependency);
        assert_eq!(code.severity(), Severity::Error);
        assert_eq!(code.message(), "dynamic include path");
    }

    #[test]
    fn reports_semantic_code_metadata() {
        let code = DiagnosticCode::ContentOutsideBlockInChildTemplate;

        assert_eq!(code.as_str(), "TERA1001");
        assert_eq!(code.kind(), DiagnosticKind::Semantic);
        assert_eq!(code.severity(), Severity::Hint);
        assert_eq!(code.message(), "content outside block in child template");
    }

    #[test]
    fn reports_function_code_metadata() {
        let code = DiagnosticCode::UnknownFunction;

        assert_eq!(code.as_str(), "TERA3002");
        assert_eq!(code.kind(), DiagnosticKind::Expression);
        assert_eq!(code.severity(), Severity::Error);
        assert_eq!(code.message(), "unknown function");
    }
}
