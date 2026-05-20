//! Structured diagnostics for Tera template source.
//!
//! Diagnostics describe violations found while parsing or analyzing Tera
//! templates. They are intended for user-facing tooling such as language
//! servers, command-line validators, formatters, and documentation generators.
//!
//! Invalid or incomplete Tera source is normal input for editor workflows.
//! Callers should be able to receive partial analysis results plus every
//! diagnostic that could be discovered.
//!
//! Diagnostic codes are stable identifiers for classes of violations. Message
//! text and help text may improve over time, but released codes should not be
//! reused for different meanings.
//!
//! # Codes
//!
//! Diagnostic codes are distinguished by range:
//!
//! - `TERA0000`-`TERA0999`: syntax and parse diagnostics
//! - `TERA1000`-`TERA1999`: single-template semantic diagnostics
//! - `TERA2000`-`TERA2999`: template dependency diagnostics
//! - `TERA3000`-`TERA3999`: expression diagnostics
//!
//! | violation | kind | severity | code |
//! | --- | --- | --- | --- |
//! | Syntax error | [syntax] | [error] | [TERA0000] |
//! | Unterminated tag | [syntax] | [error] | [TERA0001] |
//! | Unexpected end tag | [syntax] | [error] | [TERA0002] |
//! | Mismatched end tag | [syntax] | [error] | [TERA0003] |
//! | Extends not first | [semantic] | [error] | [TERA1000] |
//! | Content outside block in child template | [semantic] | [hint] | [TERA1001] |
//! | Macro not top-level | [semantic] | [error] | [TERA1002] |
//! | Block not allowed in macro | [semantic] | [error] | [TERA1003] |
//! | Extends not allowed in macro | [semantic] | [error] | [TERA1004] |
//! | Invalid template path | [dependency] | [error] | [TERA2000] |
//! | Dynamic include path | [dependency] | [error] | [TERA2001] |
//! | Unknown filter | [expression] | [error] | [TERA3000] |
//! | Unknown test | [expression] | [error] | [TERA3001] |
//! | Unknown function | [expression] | [error] | [TERA3002] |
//! | Undefined variable | [expression] | [error] | [TERA3003] |
//! | Positional macro argument | [expression] | [error] | [TERA3004] |
//! | Unknown macro namespace | [expression] | [error] | [TERA3005] |
//!
//! ## Code stability
//!
//! Diagnostic codes are part of this crate's public API.
//!
//! - Released codes keep their meaning across compatible releases.
//! - Do not reuse a removed code for a different diagnostic.
//! - Prefer adding a new code when a diagnostic splits into multiple cases.
//! - Message and help text may change over time.
//! - Tests and downstream tools should rely on codes, not exact prose.
//! - Code severity should remain stable unless changing it is intentional and
//!   documented in release notes.
//!
//! [syntax]: DiagnosticKind::Syntax
//! [semantic]: DiagnosticKind::Semantic
//! [dependency]: DiagnosticKind::Dependency
//! [expression]: DiagnosticKind::Expression
//!
//! [error]: Severity::Error
//!
//! [TERA0000]: DiagnosticCode::SyntaxError
//! [TERA0001]: DiagnosticCode::UnterminatedTag
//! [TERA0002]: DiagnosticCode::UnexpectedEndTag
//! [TERA0003]: DiagnosticCode::MismatchedEndTag
//! [TERA1000]: DiagnosticCode::ExtendsNotFirst
//! [TERA1001]: DiagnosticCode::ContentOutsideBlockInChildTemplate
//! [TERA1002]: DiagnosticCode::MacroNotTopLevel
//! [TERA1003]: DiagnosticCode::BlockNotAllowedInMacro
//! [TERA1004]: DiagnosticCode::ExtendsNotAllowedInMacro
//! [TERA2000]: DiagnosticCode::InvalidTemplatePath
//! [TERA2001]: DiagnosticCode::DynamicIncludePath
//! [TERA3000]: DiagnosticCode::UnknownFilter
//! [TERA3001]: DiagnosticCode::UnknownTest
//! [TERA3002]: DiagnosticCode::UnknownFunction
//! [TERA3003]: DiagnosticCode::UndefinedVariable
//! [TERA3004]: DiagnosticCode::PositionalMacroArgument
//! [TERA3005]: DiagnosticCode::UnknownMacroNamespace

pub use achitek_source::{Severity, TextPosition, TextRange};

/// A user-facing issue found in Tera source.
///
/// Diagnostics carry stable machine-readable metadata that downstream tools can
/// map into their own reporting formats. For example, `achitek-ls` can convert
/// this type into an LSP diagnostic without defining its own Tera diagnostic
/// codes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    code: DiagnosticCode,
    severity: Severity,
    message: String,
    help: Option<String>,
    range: TextRange,
}

impl Diagnostic {
    /// Creates a diagnostic from a code and source range.
    pub(crate) fn new(code: DiagnosticCode, range: TextRange) -> Self {
        Self {
            code,
            severity: code.severity(),
            message: code.message().to_owned(),
            help: code.help().map(str::to_owned),
            range,
        }
    }

    /// Creates a diagnostic with custom message text from a code and source
    /// range.
    #[allow(dead_code)]
    pub(crate) fn with_message(
        code: DiagnosticCode,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: code.severity(),
            message: message.into(),
            help: code.help().map(str::to_owned),
            range,
        }
    }

    /// Returns the stable diagnostic code.
    pub fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns how strongly tooling should surface this diagnostic.
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the user-facing diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns optional remediation guidance for this diagnostic.
    pub fn help(&self) -> Option<&str> {
        self.help.as_deref()
    }

    /// Returns the source range associated with this diagnostic.
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the broad analysis layer that produced this diagnostic.
    pub fn kind(&self) -> DiagnosticKind {
        self.code.kind()
    }
}

/// Broad category for a Tera diagnostic.
///
/// The kind describes which analysis layer produced a diagnostic. It is useful
/// for grouping diagnostics in docs and tests, while [`DiagnosticCode`] remains
/// the stable identifier for a specific violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticKind {
    /// A syntax or parse violation in the source text.
    Syntax,
    /// A semantic violation in syntactically valid Tera source.
    Semantic,
    /// A violation involving template references such as includes or extends.
    Dependency,
    /// A violation inside a Tera expression.
    Expression,
}

/// Stable identifiers for Tera diagnostics.
///
/// Codes are part of the public diagnostic contract for downstream tools. Once
/// released, a code should keep the same meaning. Prefer adding a new code over
/// reusing or renumbering an existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    /// `TERA0000`: the template contains syntax that tree-sitter could not parse.
    SyntaxError,
    /// `TERA0001`: a Tera variable, statement, or comment tag is not closed.
    UnterminatedTag,
    /// `TERA0002`: an end tag appears without a matching opening tag.
    UnexpectedEndTag,
    /// `TERA0003`: an end tag closes a different construct than the open tag.
    MismatchedEndTag,
    /// `TERA1000`: an `extends` statement is not the first statement.
    ExtendsNotFirst,
    /// `TERA1001`: a child template has renderable content outside a block.
    ContentOutsideBlockInChildTemplate,
    /// `TERA1002`: a macro definition appears outside the template top level.
    MacroNotTopLevel,
    /// `TERA1003`: a block definition appears inside a macro.
    BlockNotAllowedInMacro,
    /// `TERA1004`: an `extends` statement appears inside a macro.
    ExtendsNotAllowedInMacro,
    /// `TERA2000`: a template path in an include, extends, or import is invalid.
    InvalidTemplatePath,
    /// `TERA2001`: an include path is built dynamically.
    DynamicIncludePath,
    /// `TERA3000`: an expression uses a filter that is not known.
    UnknownFilter,
    /// `TERA3001`: an expression uses a test that is not known.
    UnknownTest,
    /// `TERA3002`: an expression calls a function that is not known.
    UnknownFunction,
    /// `TERA3003`: an expression references a variable that is not in scope.
    UndefinedVariable,
    /// `TERA3004`: a macro call uses a positional argument.
    PositionalMacroArgument,
    /// `TERA3005`: a macro call uses a namespace that is not in scope.
    UnknownMacroNamespace,
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl DiagnosticCode {
    /// Returns the broad diagnostic category for this code.
    pub fn kind(&self) -> DiagnosticKind {
        match self {
            Self::SyntaxError
            | Self::UnterminatedTag
            | Self::UnexpectedEndTag
            | Self::MismatchedEndTag => DiagnosticKind::Syntax,
            Self::ExtendsNotFirst
            | Self::ContentOutsideBlockInChildTemplate
            | Self::MacroNotTopLevel
            | Self::BlockNotAllowedInMacro
            | Self::ExtendsNotAllowedInMacro => DiagnosticKind::Semantic,
            Self::InvalidTemplatePath | Self::DynamicIncludePath => DiagnosticKind::Dependency,
            Self::UnknownFilter
            | Self::UnknownTest
            | Self::UnknownFunction
            | Self::UndefinedVariable
            | Self::PositionalMacroArgument
            | Self::UnknownMacroNamespace => DiagnosticKind::Expression,
        }
    }

    /// Returns the severity of the diagnostic code.
    pub fn severity(&self) -> Severity {
        match self {
            Self::SyntaxError
            | Self::UnterminatedTag
            | Self::UnexpectedEndTag
            | Self::MismatchedEndTag
            | Self::ExtendsNotFirst
            | Self::MacroNotTopLevel
            | Self::BlockNotAllowedInMacro
            | Self::ExtendsNotAllowedInMacro
            | Self::InvalidTemplatePath
            | Self::DynamicIncludePath
            | Self::UnknownFilter
            | Self::UnknownTest
            | Self::UnknownFunction
            | Self::UndefinedVariable
            | Self::PositionalMacroArgument
            | Self::UnknownMacroNamespace => Severity::Error,
            Self::ContentOutsideBlockInChildTemplate => Severity::Hint,
        }
    }

    /// Returns the stable machine-readable code.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SyntaxError => "TERA0000",
            Self::UnterminatedTag => "TERA0001",
            Self::UnexpectedEndTag => "TERA0002",
            Self::MismatchedEndTag => "TERA0003",
            Self::ExtendsNotFirst => "TERA1000",
            Self::ContentOutsideBlockInChildTemplate => "TERA1001",
            Self::MacroNotTopLevel => "TERA1002",
            Self::BlockNotAllowedInMacro => "TERA1003",
            Self::ExtendsNotAllowedInMacro => "TERA1004",
            Self::InvalidTemplatePath => "TERA2000",
            Self::DynamicIncludePath => "TERA2001",
            Self::UnknownFilter => "TERA3000",
            Self::UnknownTest => "TERA3001",
            Self::UnknownFunction => "TERA3002",
            Self::UndefinedVariable => "TERA3003",
            Self::PositionalMacroArgument => "TERA3004",
            Self::UnknownMacroNamespace => "TERA3005",
        }
    }

    /// Returns the default message for this diagnostic code.
    pub fn message(&self) -> &'static str {
        match self {
            Self::SyntaxError => "syntax error",
            Self::UnterminatedTag => "unterminated tag",
            Self::UnexpectedEndTag => "unexpected end tag",
            Self::MismatchedEndTag => "mismatched end tag",
            Self::ExtendsNotFirst => "extends statement is not first",
            Self::ContentOutsideBlockInChildTemplate => "content outside block in child template",
            Self::MacroNotTopLevel => "macro is not defined at the top level",
            Self::BlockNotAllowedInMacro => "block is not allowed in macro",
            Self::ExtendsNotAllowedInMacro => "extends is not allowed in macro",
            Self::InvalidTemplatePath => "invalid template path",
            Self::DynamicIncludePath => "dynamic include path",
            Self::UnknownFilter => "unknown filter",
            Self::UnknownTest => "unknown test",
            Self::UnknownFunction => "unknown function",
            Self::UndefinedVariable => "undefined variable",
            Self::PositionalMacroArgument => "positional macro argument",
            Self::UnknownMacroNamespace => "unknown macro namespace",
        }
    }

    /// Returns default help text for this diagnostic code.
    pub fn help(&self) -> Option<&'static str> {
        match self {
            Self::SyntaxError => Some("Check the surrounding Tera tag or expression."),
            Self::UnterminatedTag => Some("Close the tag with `}}`, `%}`, or `#}` as appropriate."),
            Self::UnexpectedEndTag => {
                Some("Remove the end tag or add the matching opening block before it.")
            }
            Self::MismatchedEndTag => Some("Use the end tag that matches the open Tera block."),
            Self::ExtendsNotFirst => {
                Some("Move the `extends` statement to the start of the child template.")
            }
            Self::ContentOutsideBlockInChildTemplate => Some(
                "Move the content into a named block; non-block content in child templates is ignored.",
            ),
            Self::MacroNotTopLevel => Some("Define macros at the top level of the template."),
            Self::BlockNotAllowedInMacro => Some("Move the block outside of the macro definition."),
            Self::ExtendsNotAllowedInMacro => {
                Some("Move `extends` to the template top level before other statements.")
            }
            Self::InvalidTemplatePath => {
                Some("Use a non-empty string literal path in `include`, `extends`, or `import`.")
            }
            Self::DynamicIncludePath => {
                Some("Use a static string path or static list of string paths in `include`.")
            }
            Self::UnknownFilter => Some("Use a registered Tera filter name."),
            Self::UnknownTest => Some("Use a registered Tera test name."),
            Self::UnknownFunction => Some("Use a registered Tera function name."),
            Self::UndefinedVariable => {
                Some("Use a variable provided by the template context or bound in this template.")
            }
            Self::PositionalMacroArgument => {
                Some("Call macros with keyword arguments such as `name=value`.")
            }
            Self::UnknownMacroNamespace => {
                Some("Use `self` or a namespace imported with an `import` statement.")
            }
        }
    }
}
