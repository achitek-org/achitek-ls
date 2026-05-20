#[cfg(test)]
mod parse {
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

#[cfg(test)]
mod analysis {
    use indoc::indoc;
    use terafile::{BindingKind, DiagnosticCode, TemplateDependencyKind, TemplatePath};

    fn diagnostic_codes(source: &str) -> Vec<&'static str> {
        terafile::analyze(source)
            .expect("expected analysis to run")
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect()
    }

    fn assert_reports(source: &str, expected: DiagnosticCode) {
        let actual = diagnostic_codes(source);
        assert!(
            actual.contains(&expected.as_str()),
            "expected diagnostic code {} in {:?}",
            expected.as_str(),
            actual
        );
    }

    #[test]
    fn recovers_template_dependencies_and_macro_shapes() {
        let source = indoc! {r#"
            {% extends "base.html" %}
            {% import "forms.html" as forms %}
            {% include ["prompt.html", "fallback.html"] ignore missing %}

            {% macro label(text, class="primary") %}
              {{ text | upper }}
            {% endmacro %}
        "#};

        let analysis = terafile::analyze(source).expect("expected analysis to run");
        let file = analysis.file();
        assert!(!analysis.has_errors());

        assert_eq!(file.dependencies().len(), 3);
        assert!(matches!(
            file.dependencies()[0].value.kind,
            TemplateDependencyKind::Extends
        ));
        assert_eq!(
            file.dependencies()[0].value.path,
            TemplatePath::Single("base.html".to_owned())
        );
        assert_eq!(
            file.dependencies()[1].value.kind,
            TemplateDependencyKind::Import {
                namespace: Some("forms".to_owned())
            }
        );
        assert_eq!(
            file.dependencies()[2].value.kind,
            TemplateDependencyKind::Include {
                ignore_missing: true
            }
        );
        assert_eq!(
            file.dependencies()[2].value.path,
            TemplatePath::Choice(vec!["prompt.html".to_owned(), "fallback.html".to_owned()])
        );

        assert_eq!(file.macros().len(), 1);
        assert_eq!(file.macros()[0].value.name, "label");
        assert_eq!(file.macros()[0].value.parameters.len(), 2);
        assert!(!file.macros()[0].value.parameters[0].value.has_default);
        assert!(file.macros()[0].value.parameters[1].value.has_default);
    }

    #[test]
    fn recovers_bindings_and_callable_references() {
        let source = indoc! {r#"
            {% import "forms.html" as forms %}
            {% set title = "Hello" %}
            {% for key, prompt in prompts | filter(attribute="visible", value=true) %}
              {{ forms::field(name=prompt.name) }}
              {{ url_for(name="home") }}
              {% if prompt.name is defined %}{{ prompt.name }}{% endif %}
            {% endfor %}
        "#};

        let analysis = terafile::analyze(source).expect("expected analysis to run");
        let file = analysis.file();
        assert!(!analysis.has_errors());

        let bindings = file
            .bindings()
            .iter()
            .map(|binding| (binding.value.name.as_str(), binding.value.kind))
            .collect::<Vec<_>>();
        assert!(bindings.contains(&("forms", BindingKind::ImportNamespace)));
        assert!(bindings.contains(&("title", BindingKind::Set)));
        assert!(bindings.contains(&("key", BindingKind::ForVariable)));
        assert!(bindings.contains(&("prompt", BindingKind::ForVariable)));
        assert!(bindings.contains(&("loop", BindingKind::LoopVariable)));

        let filters = file
            .filters()
            .iter()
            .map(|filter| filter.value.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(filters, vec!["filter"]);

        let tests = file
            .tests()
            .iter()
            .map(|test| test.value.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(tests, vec!["defined"]);

        let functions = file
            .functions()
            .iter()
            .map(|function| function.value.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(functions, vec!["url_for"]);

        assert_eq!(file.macro_calls().len(), 1);
        assert_eq!(file.macro_calls()[0].value.namespace, "forms");
        assert_eq!(file.macro_calls()[0].value.name, "field");
        assert_eq!(file.macro_calls()[0].value.arguments[0].value.name, "name");

        let references = file
            .variable_references()
            .iter()
            .map(|reference| reference.value.path.as_str())
            .collect::<Vec<_>>();
        assert!(references.contains(&"prompts"));
        assert!(references.contains(&"prompt.name"));
    }

    #[test]
    fn reports_syntax_error_for_recovered_error_node() {
        assert_reports("{% if enabled %}{{ name }}", DiagnosticCode::SyntaxError);
    }

    #[test]
    fn reports_unterminated_tag() {
        assert_reports("{{ name", DiagnosticCode::UnterminatedTag);
    }

    #[test]
    fn reports_unexpected_end_tag() {
        assert_reports("{% endif %}", DiagnosticCode::UnexpectedEndTag);
    }

    #[test]
    fn reports_extends_not_first() {
        assert_reports(
            indoc! {r#"
                <header></header>
                {% extends "base.html" %}
            "#},
            DiagnosticCode::ExtendsNotFirst,
        );
    }

    #[test]
    fn reports_content_outside_block_in_child_template() {
        assert_reports(
            indoc! {r#"
                {% extends "base.html" %}

                ignored content

                {% block content %}Hello{% endblock %}
            "#},
            DiagnosticCode::ContentOutsideBlockInChildTemplate,
        );
    }

    #[test]
    fn reports_macro_not_top_level() {
        assert_reports(
            indoc! {r#"
                {% if enabled %}
                  {% macro label(text) %}{{ text }}{% endmacro %}
                {% endif %}
            "#},
            DiagnosticCode::MacroNotTopLevel,
        );
    }

    #[test]
    fn reports_block_not_allowed_in_macro() {
        assert_reports(
            indoc! {r#"
                {% macro label(text) %}
                  {% block content %}{{ text }}{% endblock %}
                {% endmacro %}
            "#},
            DiagnosticCode::BlockNotAllowedInMacro,
        );
    }

    #[test]
    fn reports_extends_not_allowed_in_macro() {
        assert_reports(
            indoc! {r#"
                {% macro label(text) %}
                  {% extends "base.html" %}
                {% endmacro %}
            "#},
            DiagnosticCode::ExtendsNotAllowedInMacro,
        );
    }

    #[test]
    fn reports_dynamic_include_path() {
        assert_reports(
            "{% include template_name %}",
            DiagnosticCode::DynamicIncludePath,
        );
    }
}
