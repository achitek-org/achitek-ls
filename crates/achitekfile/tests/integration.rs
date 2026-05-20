#[cfg(test)]
mod parse {
    use indoc::indoc;

    #[test]
    fn parses_file_and_returns_tree() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
            }
        "#};

        let tree = achitekfile::parse(source).expect("expected source to parse");
        let root = tree.root_node();

        assert_eq!(root.kind(), "file");
        assert!(!root.has_error());
    }

    #[test]
    fn parses_invalid_source_with_error_nodes() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app
            }
        "#};

        let tree = achitekfile::parse(source).expect("expected tree-sitter to recover a tree");
        let root = tree.root_node();

        assert_eq!(root.kind(), "file");
        assert!(root.has_error());
    }
}

#[cfg(test)]
mod analysis {
    use achitekfile::{
        DiagnosticCode,
        model::{PromptType, Value},
    };
    use indoc::indoc;

    fn diagnostic_codes(source: &str) -> Vec<&'static str> {
        achitekfile::analyze(source)
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

    fn assert_does_not_report(source: &str, unexpected: DiagnosticCode) {
        let actual = diagnostic_codes(source);
        assert!(
            !actual.contains(&unexpected.as_str()),
            "unexpected diagnostic code {} in {:?}",
            unexpected.as_str(),
            actual
        );
    }

    macro_rules! diagnostic_test {
        ($name:ident, $code:expr, $source:expr) => {
            #[test]
            fn $name() {
                assert_reports($source, $code);
            }
        };
    }

    diagnostic_test!(
        reports_missing_blueprint_block,
        DiagnosticCode::MissingBlueprintBlock,
        indoc! {r#"
            prompt "project" {
              type = string
            }
        "#}
    );

    diagnostic_test!(
        reports_multiple_blueprint_blocks,
        DiagnosticCode::MultipleBlueprintBlocks,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            blueprint {
              version = "1.0.0"
              name = "other"
            }
        "#}
    );

    diagnostic_test!(
        reports_prompt_before_blueprint,
        DiagnosticCode::PromptBeforeBlueprint,
        indoc! {r#"
            prompt "project" {
              type = string
            }

            blueprint {
              version = "1.0.0"
              name = "web-app"
            }
        "#}
    );

    diagnostic_test!(
        reports_unknown_top_level_item,
        DiagnosticCode::UnknownTopLevelItem,
        indoc! {r#"
            wat
        "#}
    );

    diagnostic_test!(
        reports_unknown_blueprint_attribute,
        DiagnosticCode::UnknownBlueprintAttribute,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
              unknown = "value"
            }
        "#}
    );

    diagnostic_test!(
        reports_unknown_prompt_attribute,
        DiagnosticCode::UnknownPromptAttribute,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              unknown = "value"
            }
        "#}
    );

    diagnostic_test!(
        reports_unknown_validate_attribute,
        DiagnosticCode::UnknownValidateAttribute,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              validate {
                unknown = 1
              }
            }
        "#}
    );

    diagnostic_test!(
        reports_unknown_prompt_type,
        DiagnosticCode::UnknownPromptType,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = not_a_type
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_boolean_literal,
        DiagnosticCode::InvalidBooleanLiteral,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = bool
              required = maybe
            }
        "#}
    );

    diagnostic_test!(
        reports_unterminated_string,
        DiagnosticCode::UnterminatedString,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_escape_sequence,
        DiagnosticCode::InvalidEscapeSequence,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web\q-app"
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_dependency_expression,
        DiagnosticCode::InvalidDependencyExpression,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              depends_on = _invalid
            }
        "#}
    );

    diagnostic_test!(
        reports_unknown_dependency_method,
        DiagnosticCode::UnknownDependencyMethod,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "features" {
              type = multiselect
              choices = ["x"]
            }

            prompt "project" {
              type = string
              depends_on = features.includes("x")
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_identifier,
        DiagnosticCode::InvalidIdentifier,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              depends_on = all()
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_integer,
        DiagnosticCode::InvalidInteger,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              validate {
                min_length = -1
              }
            }
        "#}
    );

    diagnostic_test!(
        reports_malformed_array,
        DiagnosticCode::MalformedArray,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = select
              choices = ["bin" "lib"]
            }
        "#}
    );

    diagnostic_test!(
        reports_missing_prompt_name,
        DiagnosticCode::MissingPromptName,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt {
              type = string
            }
        "#}
    );

    diagnostic_test!(
        reports_missing_attribute_value,
        DiagnosticCode::MissingAttributeValue,
        indoc! {r#"
            blueprint {
              version =
              name = "web-app"
            }
        "#}
    );

    diagnostic_test!(
        reports_missing_blueprint_version,
        DiagnosticCode::MissingBlueprintVersion,
        indoc! {r#"
            blueprint {
              name = "web-app"
            }
        "#}
    );

    diagnostic_test!(
        reports_missing_blueprint_name,
        DiagnosticCode::MissingBlueprintName,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
            }
        "#}
    );

    diagnostic_test!(
        reports_empty_blueprint_name,
        DiagnosticCode::EmptyBlueprintName,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = ""
            }
        "#}
    );

    diagnostic_test!(
        reports_empty_blueprint_version,
        DiagnosticCode::EmptyBlueprintVersion,
        indoc! {r#"
            blueprint {
              version = ""
              name = "web-app"
            }
        "#}
    );

    diagnostic_test!(
        reports_duplicate_blueprint_attribute,
        DiagnosticCode::DuplicateBlueprintAttribute,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              version = "1.0.1"
              name = "web-app"
            }
        "#}
    );

    diagnostic_test!(
        reports_missing_prompt_type,
        DiagnosticCode::MissingPromptType,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              help = "Project name"
            }
        "#}
    );

    diagnostic_test!(
        reports_empty_prompt_name,
        DiagnosticCode::EmptyPromptName,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "" {
              type = string
            }
        "#}
    );

    diagnostic_test!(
        reports_duplicate_prompt_name,
        DiagnosticCode::DuplicatePromptName,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
            }

            prompt "project" {
              type = string
            }
        "#}
    );

    diagnostic_test!(
        reports_duplicate_prompt_attribute,
        DiagnosticCode::DuplicatePromptAttribute,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              help = "one"
              help = "two"
            }
        "#}
    );

    diagnostic_test!(
        reports_duplicate_validate_attribute,
        DiagnosticCode::DuplicateValidateAttribute,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              validate {
                min_length = 1
                min_length = 2
              }
            }
        "#}
    );

    diagnostic_test!(
        reports_choices_on_non_choice_prompt,
        DiagnosticCode::ChoicesOnNonChoicePrompt,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              choices = ["x"]
            }
        "#}
    );

    diagnostic_test!(
        reports_missing_choices_for_select,
        DiagnosticCode::MissingChoicesForSelect,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "kind" {
              type = select
            }
        "#}
    );

    diagnostic_test!(
        reports_missing_choices_for_multiselect,
        DiagnosticCode::MissingChoicesForMultiselect,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "features" {
              type = multiselect
            }
        "#}
    );

    diagnostic_test!(
        reports_empty_choices_list,
        DiagnosticCode::EmptyChoicesList,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "kind" {
              type = select
              choices = []
            }
        "#}
    );

    #[test]
    fn empty_choices_for_select_reports_empty_choices_only() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "kind" {
              type = select
              choices = []
            }
        "#};
        let actual = diagnostic_codes(source);

        assert!(actual.contains(&DiagnosticCode::EmptyChoicesList.as_str()));
        assert!(!actual.contains(&DiagnosticCode::MissingChoicesForSelect.as_str()));
    }

    diagnostic_test!(
        reports_duplicate_choice,
        DiagnosticCode::DuplicateChoice,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "kind" {
              type = select
              choices = ["bin", "bin"]
            }
        "#}
    );

    diagnostic_test!(
        reports_non_string_choice,
        DiagnosticCode::NonStringChoice,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "kind" {
              type = select
              choices = ["bin", 1]
            }
        "#}
    );

    diagnostic_test!(
        reports_default_type_mismatch,
        DiagnosticCode::DefaultTypeMismatch,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "kind" {
              type = select
              choices = ["bin"]
              default = true
            }
        "#}
    );

    diagnostic_test!(
        reports_select_default_not_in_choices,
        DiagnosticCode::SelectDefaultNotInChoices,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "kind" {
              type = select
              choices = ["bin"]
              default = "lib"
            }
        "#}
    );

    diagnostic_test!(
        reports_multiselect_default_must_be_array,
        DiagnosticCode::MultiselectDefaultMustBeArray,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "features" {
              type = multiselect
              choices = ["auth"]
              default = "auth"
            }
        "#}
    );

    diagnostic_test!(
        reports_multiselect_default_contains_unknown_choice,
        DiagnosticCode::MultiselectDefaultContainsUnknownChoice,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "features" {
              type = multiselect
              choices = ["auth"]
              default = ["db"]
            }
        "#}
    );

    diagnostic_test!(
        reports_required_false_with_no_default,
        DiagnosticCode::RequiredFalseWithNoDefault,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              required = false
            }
        "#}
    );

    diagnostic_test!(
        reports_duplicate_validate_block,
        DiagnosticCode::DuplicateValidateBlock,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              validate {
                min_length = 1
              }
              validate {
                max_length = 10
              }
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_blueprint_version,
        DiagnosticCode::InvalidBlueprintVersion,
        indoc! {r#"
            blueprint {
              version = "bad"
              name = "web-app"
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_minimum_achitek_version,
        DiagnosticCode::InvalidMinimumAchitekVersion,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
              min_achitek_version = "bad"
            }
        "#}
    );

    diagnostic_test!(
        reports_unknown_dependency_reference,
        DiagnosticCode::UnknownDependencyReference,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              depends_on = missing
            }
        "#}
    );

    diagnostic_test!(
        reports_self_dependency,
        DiagnosticCode::SelfDependency,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              depends_on = project
            }
        "#}
    );

    diagnostic_test!(
        reports_dependency_cycle,
        DiagnosticCode::DependencyCycle,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "a" {
              type = string
              depends_on = b
            }

            prompt "b" {
              type = string
              depends_on = a
            }
        "#}
    );

    diagnostic_test!(
        reports_dependency_type_mismatch,
        DiagnosticCode::DependencyTypeMismatch,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "flag" {
              type = bool
            }

            prompt "project" {
              type = string
              depends_on = flag == "yes"
            }
        "#}
    );

    diagnostic_test!(
        reports_contains_on_non_multiselect_prompt,
        DiagnosticCode::ContainsOnNonMultiselectPrompt,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "flag" {
              type = bool
            }

            prompt "project" {
              type = string
              depends_on = flag.contains("yes")
            }
        "#}
    );

    diagnostic_test!(
        reports_contains_unknown_choice,
        DiagnosticCode::ContainsUnknownChoice,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "features" {
              type = multiselect
              choices = ["auth"]
            }

            prompt "project" {
              type = string
              depends_on = features.contains("db")
            }
        "#}
    );

    diagnostic_test!(
        reports_string_validation_on_non_string_prompt,
        DiagnosticCode::StringValidationOnNonStringPrompt,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "flag" {
              type = bool
              validate {
                min_length = 1
              }
            }
        "#}
    );

    diagnostic_test!(
        reports_selection_validation_on_non_multiselect_prompt,
        DiagnosticCode::SelectionValidationOnNonMultiselectPrompt,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              validate {
                min_selections = 1
              }
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_length_bounds,
        DiagnosticCode::InvalidLengthBounds,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              validate {
                min_length = 10
                max_length = 1
              }
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_selection_bounds,
        DiagnosticCode::InvalidSelectionBounds,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "features" {
              type = multiselect
              choices = ["auth"]
              validate {
                min_selections = 2
                max_selections = 1
              }
            }
        "#}
    );

    diagnostic_test!(
        reports_invalid_regex,
        DiagnosticCode::InvalidRegex,
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              validate {
                regex = "["
              }
            }
        "#}
    );

    #[test]
    fn analyze_returns_no_diagnostics_for_valid_source() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
              description = "Scaffold a web app"
              author = "Achitek"
              min_achitek_version = "0.1.0"
            }

            prompt "project" {
              type = string
              help = "Project name"
              default = "demo"
            }

            prompt "kind" {
              type = select
              choices = ["bin", "lib"]
              default = "bin"
            }

            prompt "features" {
              type = multiselect
              choices = ["auth", "db"]
              default = ["auth"]
              validate {
                min_selections = 1
                max_selections = 2
              }
            }

            prompt "orm" {
              type = select
              choices = ["sqlx", "diesel"]
              depends_on = features.contains("db")
            }
        "#};

        let analysis = achitekfile::analyze(source).expect("expected analysis to run");

        assert!(analysis.diagnostics().is_empty());
        assert!(!analysis.has_errors());
    }

    #[test]
    fn keywords_in_strings_do_not_trigger_file_shape_diagnostics() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
              description = "prompt { blueprint"
            }

            prompt "blueprint" {
              type = string
              help = "prompt {"
            }
        "#};

        assert_does_not_report(source, DiagnosticCode::MultipleBlueprintBlocks);
        assert_does_not_report(source, DiagnosticCode::PromptBeforeBlueprint);
        assert_does_not_report(source, DiagnosticCode::MissingPromptName);
    }

    #[test]
    fn includes_in_string_does_not_trigger_unknown_dependency_method() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              help = "JavaScript has .includes("
            }
        "#};

        assert_does_not_report(source, DiagnosticCode::UnknownDependencyMethod);
    }

    #[test]
    fn empty_choices_on_non_choice_prompt_reports_non_choice_usage_only() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              type = string
              choices = []
            }
        "#};
        let actual = diagnostic_codes(source);

        assert!(actual.contains(&DiagnosticCode::ChoicesOnNonChoicePrompt.as_str()));
        assert!(!actual.contains(&DiagnosticCode::EmptyChoicesList.as_str()));
    }

    #[test]
    fn into_valid_returns_valid_model_for_valid_source() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
              description = "Scaffold a web app"
              author = "Achitek"
              min_achitek_version = "0.1.0"
            }

            prompt "project" {
              type = string
              help = "Project name"
              default = "demo"
              required = true
            }

            prompt "kind" {
              type = select
              choices = ["bin", "lib"]
              default = "bin"
            }
        "#};

        let file = achitekfile::analyze(source)
            .expect("expected analysis to run")
            .into_valid()
            .expect("expected valid model");

        assert_eq!(file.blueprint().version, "1.0.0");
        assert_eq!(file.blueprint().name, "web-app");
        assert_eq!(
            file.blueprint().description.as_deref(),
            Some("Scaffold a web app")
        );
        assert_eq!(file.blueprint().author.as_deref(), Some("Achitek"));
        assert_eq!(
            file.blueprint().min_achitek_version.as_deref(),
            Some("0.1.0")
        );
        assert_eq!(file.prompts().len(), 2);
        assert_eq!(file.prompts()[0].name, "project");
        assert_eq!(file.prompts()[0].prompt_type, PromptType::String);
        assert_eq!(file.prompts()[0].help.as_deref(), Some("Project name"));
        assert_eq!(
            file.prompts()[0].default,
            Some(Value::String("demo".to_owned()))
        );
        assert!(file.prompts()[0].required);
        assert_eq!(file.prompts()[1].name, "kind");
        assert_eq!(file.prompts()[1].prompt_type, PromptType::Select);
        assert_eq!(
            file.prompts()[1].choices,
            vec![
                Value::String("bin".to_owned()),
                Value::String("lib".to_owned())
            ]
        );
    }

    #[test]
    fn into_valid_returns_diagnostics_for_syntax_errors() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app
            }
        "#};

        let diagnostics = achitekfile::analyze(source)
            .expect("expected analysis to run")
            .into_valid()
            .expect_err("expected diagnostics");
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<Vec<_>>();

        assert!(codes.contains(&DiagnosticCode::UnterminatedString.as_str()));
    }

    #[test]
    fn into_valid_returns_diagnostics_for_missing_blueprint_fields() {
        let source = indoc! {r#"
            blueprint {
            }
        "#};

        let diagnostics = achitekfile::analyze(source)
            .expect("expected analysis to run")
            .into_valid()
            .expect_err("expected diagnostics");
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<Vec<_>>();

        assert!(codes.contains(&DiagnosticCode::MissingBlueprintVersion.as_str()));
        assert!(codes.contains(&DiagnosticCode::MissingBlueprintName.as_str()));
    }

    #[test]
    fn into_valid_returns_diagnostics_for_missing_prompt_type() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "web-app"
            }

            prompt "project" {
              help = "Project name"
            }
        "#};

        let diagnostics = achitekfile::analyze(source)
            .expect("expected analysis to run")
            .into_valid()
            .expect_err("expected diagnostics");
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<Vec<_>>();

        assert!(codes.contains(&DiagnosticCode::MissingPromptType.as_str()));
    }
}
