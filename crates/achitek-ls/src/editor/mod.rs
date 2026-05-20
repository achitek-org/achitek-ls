mod completion;
mod definition;
mod hover;
mod navigation;
mod reference;
mod rename;
mod shared;

pub use completion::{Completion, CompletionKind};
pub use definition::DefinitionTarget;
pub use hover::Hover;
pub use reference::ReferenceTarget;
pub use rename::PrepareRenameTarget;

use achitekfile::{ParseError, TextPosition, TextRange};
use tree_sitter::{Node, Tree};

#[derive(Debug)]
pub struct EditorBuffer {
    syntax: SourceTree,
    prompt_declarations: Vec<navigation::PromptDeclaration>,
    symbols: Vec<Symbol>,
}

impl EditorBuffer {
    /// Returns the parsed syntax tree for the document.
    pub fn syntax(&self) -> &SourceTree {
        &self.syntax
    }

    /// Returns document symbols derived from the parsed source.
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Returns hover information for a position in the document.
    pub fn hover(&self, position: TextPosition) -> Option<Hover> {
        hover::hover_for_position(&self.syntax, position)
    }

    /// Returns completion items for a position in the document.
    pub fn completions(&self, position: TextPosition) -> Vec<Completion> {
        completion::completions_for_position(&self.syntax, &self.symbols, position)
    }

    /// Returns the definition target for a position in the document.
    pub fn definition(&self, position: TextPosition) -> Option<DefinitionTarget> {
        navigation::definition_for_position(&self.syntax, &self.prompt_declarations, position)
    }

    /// Returns rename preparation details for a position in the document.
    pub fn prepare_rename(&self, position: TextPosition) -> Option<PrepareRenameTarget> {
        navigation::prepare_rename_for_position(&self.syntax, &self.prompt_declarations, position)
    }

    /// Returns all reference targets related to the symbol under the cursor.
    pub fn references(
        &self,
        position: TextPosition,
        include_declaration: bool,
    ) -> Vec<ReferenceTarget> {
        navigation::references_for_position(
            &self.syntax,
            &self.prompt_declarations,
            position,
            include_declaration,
        )
    }

    /// Returns the prompt name associated with the symbol under the cursor.
    pub fn prompt_name(&self, position: TextPosition) -> Option<String> {
        navigation::prompt_name_at_position(&self.syntax, position, &self.prompt_declarations)
    }
}

/// Parsed source plus the Tree-sitter tree used by editor features.
#[derive(Debug)]
pub struct SourceTree {
    source: String,
    tree: Tree,
}

impl SourceTree {
    /// Returns the original source used to build this syntax tree.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the raw Tree-sitter tree.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Returns the root CST node.
    pub fn root_node(&self) -> Node<'_> {
        self.tree.root_node()
    }

    /// Returns the source range occupied by a given node.
    pub fn range_for(&self, node: Node<'_>) -> TextRange {
        TextRange {
            start: TextPosition {
                line: node.start_position().row,
                byte: node.start_position().column,
            },
            end: TextPosition {
                line: node.end_position().row,
                byte: node.end_position().column,
            },
        }
    }

    /// Returns the source text covered by a given node.
    pub fn text_for<'a>(&'a self, node: Node<'_>) -> &'a str {
        &self.source[node.byte_range()]
    }
}

/// A document symbol derived from Achitek source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    name: String,
    detail: Option<String>,
    kind: SymbolKind,
    range: TextRange,
    selection_range: TextRange,
    children: Vec<Symbol>,
}

impl Symbol {
    /// Returns the symbol display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns optional symbol detail text.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// Returns the symbol kind.
    pub fn kind(&self) -> SymbolKind {
        self.kind
    }

    /// Returns the full source range occupied by the symbol.
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the preferred selection range for the symbol.
    pub fn selection_range(&self) -> TextRange {
        self.selection_range
    }

    /// Returns nested child symbols.
    pub fn children(&self) -> &[Symbol] {
        &self.children
    }
}

/// Symbol kinds understood by editor features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// The top-level blueprint block.
    Blueprint,
    /// A prompt block.
    Prompt,
    /// A validate block nested inside a prompt.
    Validate,
}

/// Builds editor features for a single Achitek source document.
pub fn build(source: &str) -> Result<EditorBuffer, ParseError> {
    from_source(source)
}

/// Builds editor features for a single Achitek source document.
pub fn from_source(source: &str) -> Result<EditorBuffer, ParseError> {
    let tree = achitekfile::parse(source)?;
    let analysis =
        achitekfile::analyze(source).expect("analysis should not fail after parsing succeeds");
    let syntax = SourceTree {
        source: source.to_owned(),
        tree,
    };
    let prompt_declarations = navigation::collect_prompt_declarations(&syntax, &analysis);
    let symbols = collect_symbols(&syntax, &analysis);

    Ok(EditorBuffer {
        syntax,
        prompt_declarations,
        symbols,
    })
}

fn collect_symbols(syntax: &SourceTree, analysis: &achitekfile::Analysis<'_>) -> Vec<Symbol> {
    let mut symbols = Vec::new();

    if let Some(range) = analysis.file().blueprint().range {
        symbols.push(Symbol {
            name: "blueprint".to_owned(),
            detail: None,
            kind: SymbolKind::Blueprint,
            range,
            selection_range: range,
            children: Vec::new(),
        });
    }

    for prompt in analysis.file().prompts() {
        symbols.push(prompt_symbol(syntax, prompt));
    }

    symbols
}

fn prompt_symbol(
    syntax: &SourceTree,
    prompt: &achitekfile::model::Spanned<achitekfile::model::Prompt>,
) -> Symbol {
    let prompt_block = shared::prompt_block_for_range(syntax, prompt.range);
    let selection_range = prompt_block
        .and_then(|node| node.child_by_field_name("name"))
        .map(|node| syntax.range_for(node))
        .unwrap_or(prompt.range);
    let children = prompt_block
        .map(|node| collect_prompt_children(syntax, node))
        .unwrap_or_default();

    Symbol {
        name: prompt.value.name.clone(),
        detail: Some("prompt".to_owned()),
        kind: SymbolKind::Prompt,
        range: prompt.range,
        selection_range,
        children,
    }
}

fn collect_prompt_children(syntax: &SourceTree, prompt_node: Node<'_>) -> Vec<Symbol> {
    let mut children = Vec::new();

    for index in 0..prompt_node.child_count() {
        let Some(child) =
            prompt_node.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };

        if child.kind() == "validate_block" {
            let range = syntax.range_for(child);
            children.push(Symbol {
                name: "validate".to_owned(),
                detail: None,
                kind: SymbolKind::Validate,
                range,
                selection_range: range,
                children: Vec::new(),
            });
        }
    }

    children
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn filters_prompt_attribute_completions_by_type_and_existing_attributes() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "name" {
              type = string

            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let completions = analysis.completions(TextPosition { line: 7, byte: 2 });

        assert!(!completions.iter().any(|item| item.label() == "type"));
        assert!(!completions.iter().any(|item| item.label() == "choices"));
        assert!(completions.iter().any(|item| item.label() == "default"));
    }

    #[test]
    fn filters_validate_completions_by_prompt_type() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "kind" {
              type = multiselect
              choices = ["a", "b"]

              validate {

              }
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let completions = analysis.completions(TextPosition { line: 10, byte: 4 });

        assert!(
            completions
                .iter()
                .any(|item| item.label() == "min_selections")
        );
        assert!(!completions.iter().any(|item| item.label() == "min_length"));
    }

    #[test]
    fn collects_document_symbols() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"

              validate {
                min_length = 2
              }
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");

        assert_eq!(analysis.symbols().len(), 2);
        assert_eq!(analysis.symbols()[0].name(), "blueprint");
        assert_eq!(analysis.symbols()[0].kind(), SymbolKind::Blueprint);
        assert_eq!(analysis.symbols()[1].name(), "project_name");
        assert_eq!(analysis.symbols()[1].kind(), SymbolKind::Prompt);
        assert_eq!(analysis.symbols()[1].children().len(), 1);
        assert_eq!(analysis.symbols()[1].children()[0].name(), "validate");
        assert_eq!(
            analysis.symbols()[1].children()[0].kind(),
            SymbolKind::Validate
        );
    }

    #[test]
    fn returns_hover_for_prompt_type() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let hover = analysis
            .hover(TextPosition { line: 6, byte: 9 })
            .expect("hover should exist for prompt type");

        assert!(hover.contents().contains("`string`"));
        assert!(hover.contents().contains("single-line text prompt"));
    }

    #[test]
    fn returns_prompt_type_completions() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = 
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let completions = analysis.completions(TextPosition { line: 6, byte: 9 });

        assert!(completions.iter().any(|item| item.label() == "string"));
        assert!(completions.iter().any(|item| item.label() == "paragraph"));
    }

    #[test]
    fn returns_depends_on_completions() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }

            prompt "kind" {
              type = string
              help = "Kind"
              depends_on = 
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let completions = analysis.completions(TextPosition { line: 13, byte: 15 });

        assert!(completions.iter().any(|item| item.label() == "all"));
        assert!(
            completions
                .iter()
                .any(|item| item.label() == "project_name")
        );
    }

    #[test]
    fn resolves_definition_for_prompt_reference() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }

            prompt "kind" {
              type = string
              help = "Kind"
              depends_on = project_name
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let definition = analysis
            .definition(TextPosition { line: 13, byte: 16 })
            .expect("definition should exist for prompt reference");

        assert_eq!(definition.selection_range().start.line, 5);
        assert_eq!(definition.selection_range().start.byte, 7);
    }

    #[test]
    fn finds_references_for_prompt_definition() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }

            prompt "kind" {
              type = string
              help = "Kind"
              depends_on = project_name
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let references = analysis.references(TextPosition { line: 5, byte: 9 }, true);

        assert_eq!(references.len(), 2);
        assert_eq!(references[0].range().start.line, 5);
        assert_eq!(references[1].range().start.line, 13);
    }

    #[test]
    fn prepares_rename_for_prompt_definition() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let target = analysis
            .prepare_rename(TextPosition { line: 5, byte: 10 })
            .expect("prepare rename should exist for prompt definition");

        assert_eq!(target.placeholder(), "project_name");
        assert_eq!(target.range().start.line, 5);
        assert_eq!(target.range().start.byte, 7);
    }

    #[test]
    fn prepares_rename_for_prompt_reference() {
        let source = indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
            }

            prompt "kind" {
              type = string
              depends_on = project_name
            }
        "#};

        let analysis = from_source(source).expect("valid source should build");
        let target = analysis
            .prepare_rename(TextPosition { line: 11, byte: 16 })
            .expect("prepare rename should exist for prompt reference");

        assert_eq!(target.placeholder(), "project_name");
        assert_eq!(target.range().start.line, 11);
        assert_eq!(target.range().start.byte, 15);
    }
}
