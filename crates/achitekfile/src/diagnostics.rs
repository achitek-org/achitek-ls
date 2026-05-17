//! Structured diagnostics for Achitekfile source violations.
//!
//! Diagnostics describe violations found while parsing or analyzing
//! Achitekfile source. They are intended for user-facing tooling such as
//! language servers, command-line validators, formatters, and documentation
//! generators.
//!
//! A diagnostic is different from a fatal Rust error. Invalid Achitekfile source
//! is normal input for editor and validation workflows, so callers should be
//! able to receive a partial analysis result plus every diagnostic that could
//! be discovered.
//!
//! Diagnostic codes are stable identifiers for classes of violations. Message
//! text and help text may improve over time, but released codes should not be
//! reused for different meanings.
//!
//! # Codes
//!
//! Diagnostic codes are distinguished by range:
//!
//! - `ACH0000`-`ACH0999`: syntax and parse diagnostics
//! - `ACH1000`-`ACH1999`: single-file semantic diagnostics
//! - `ACH2000`-`ACH2999`: dependency graph diagnostics
//! - `ACH3000`-`ACH3999`: validation rule diagnostics
//!
//! | violation | kind | severity | code |
//! | --- | --- | --- | --- |
//! | Missing blueprint block | [syntax] | [error] | [ACH0000] |
//! | Multiple blueprint blocks | [syntax] | [error] | [ACH0001] |
//! | Prompt before blueprint | [syntax] | [error] | [ACH0002] |
//! | Unknown top-level item | [syntax] | [error] | [ACH0003] |
//! | Unknown blueprint attribute | [syntax] | [error] | [ACH0004] |
//! | Unknown prompt attribute | [syntax] | [error] | [ACH0005] |
//! | Unknown validate attribute | [syntax] | [error] | [ACH0006] |
//! | Unknown prompt type | [syntax] | [error] | [ACH0007] |
//! | Invalid boolean literal | [syntax] | [error] | [ACH0008] |
//! | Unterminated string | [syntax] | [error] | [ACH0009] |
//! | Invalid escape sequence | [syntax] | [error] | [ACH0010] |
//! | Invalid dependency expression | [syntax] | [error] | [ACH0011] |
//! | Unknown dependency method | [syntax] | [error] | [ACH0012] |
//! | Invalid identifier | [syntax] | [error] | [ACH0013] |
//! | Invalid integer | [syntax] | [error] | [ACH0014] |
//! | Malformed array | [syntax] | [error] | [ACH0015] |
//! | Missing prompt name | [syntax] | [error] | [ACH0016] |
//! | Missing attribute value | [syntax] | [error] | [ACH0017] |
//! | Missing blueprint version | [semantic] | [error] | [ACH1000] |
//! | Missing blueprint name | [semantic] | [error] | [ACH1001] |
//! | Empty blueprint name | [semantic] | [error] | [ACH1002] |
//! | Empty blueprint version | [semantic] | [error] | [ACH1003] |
//! | Duplicate blueprint attribute | [semantic] | [error] | [ACH1004] |
//! | Missing prompt type | [semantic] | [error] | [ACH1005] |
//! | Empty prompt name | [semantic] | [error] | [ACH1006] |
//! | Duplicate prompt name | [semantic] | [error] | [ACH1007] |
//! | Duplicate prompt attribute | [semantic] | [error] | [ACH1008] |
//! | Duplicate validate attribute | [semantic] | [error] | [ACH1009] |
//! | Choices on non-choice prompt | [semantic] | [error] | [ACH1010] |
//! | Missing choices for select | [semantic] | [error] | [ACH1011] |
//! | Missing choices for multiselect | [semantic] | [error] | [ACH1012] |
//! | Empty choices list | [semantic] | [error] | [ACH1013] |
//! | Duplicate choice | [semantic] | [warning] | [ACH1014] |
//! | Non-string choice | [semantic] | [error] | [ACH1015] |
//! | Default type mismatch | [semantic] | [error] | [ACH1016] |
//! | Select default not in choices | [semantic] | [error] | [ACH1017] |
//! | Multiselect default must be array | [semantic] | [error] | [ACH1018] |
//! | Multiselect default contains unknown choice | [semantic] | [error] | [ACH1019] |
//! | Required false with no default | [semantic] | [hint] | [ACH1020] |
//! | Duplicate validate block | [semantic] | [error] | [ACH1021] |
//! | Invalid blueprint version | [semantic] | [error] | [ACH1022] |
//! | Invalid minimum Achitek version | [semantic] | [error] | [ACH1023] |
//! | Dependency references unknown prompt | [dependency] | [error] | [ACH2000] |
//! | Dependency references itself | [dependency] | [error] | [ACH2001] |
//! | Dependency cycle | [dependency] | [error] | [ACH2002] |
//! | Dependency type mismatch | [dependency] | [error] | [ACH2003] |
//! | Contains on non-multiselect prompt | [dependency] | [error] | [ACH2004] |
//! | Contains unknown choice | [dependency] | [error] | [ACH2005] |
//! | String validation on non-string prompt | [validation] | [error] | [ACH3000] |
//! | Selection validation on non-multiselect prompt | [validation] | [error] | [ACH3001] |
//! | Invalid length bounds | [validation] | [error] | [ACH3002] |
//! | Invalid selection bounds | [validation] | [error] | [ACH3003] |
//! | Invalid regex | [validation] | [error] | [ACH3004] |
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
//! [validation]: DiagnosticKind::Validation
//!
//! [error]: Severity::Error
//! [warning]: Severity::Warning
//! [hint]: Severity::Hint
//!
//! [ACH0000]: DiagnosticCode::MissingBlueprintBlock
//! [ACH0001]: DiagnosticCode::MultipleBlueprintBlocks
//! [ACH0002]: DiagnosticCode::PromptBeforeBlueprint
//! [ACH0003]: DiagnosticCode::UnknownTopLevelItem
//! [ACH0004]: DiagnosticCode::UnknownBlueprintAttribute
//! [ACH0005]: DiagnosticCode::UnknownPromptAttribute
//! [ACH0006]: DiagnosticCode::UnknownValidateAttribute
//! [ACH0007]: DiagnosticCode::UnknownPromptType
//! [ACH0008]: DiagnosticCode::InvalidBooleanLiteral
//! [ACH0009]: DiagnosticCode::UnterminatedString
//! [ACH0010]: DiagnosticCode::InvalidEscapeSequence
//! [ACH0011]: DiagnosticCode::InvalidDependencyExpression
//! [ACH0012]: DiagnosticCode::UnknownDependencyMethod
//! [ACH0013]: DiagnosticCode::InvalidIdentifier
//! [ACH0014]: DiagnosticCode::InvalidInteger
//! [ACH0015]: DiagnosticCode::MalformedArray
//! [ACH0016]: DiagnosticCode::MissingPromptName
//! [ACH0017]: DiagnosticCode::MissingAttributeValue
//! [ACH1000]: DiagnosticCode::MissingBlueprintVersion
//! [ACH1001]: DiagnosticCode::MissingBlueprintName
//! [ACH1002]: DiagnosticCode::EmptyBlueprintName
//! [ACH1003]: DiagnosticCode::EmptyBlueprintVersion
//! [ACH1004]: DiagnosticCode::DuplicateBlueprintAttribute
//! [ACH1005]: DiagnosticCode::MissingPromptType
//! [ACH1006]: DiagnosticCode::EmptyPromptName
//! [ACH1007]: DiagnosticCode::DuplicatePromptName
//! [ACH1008]: DiagnosticCode::DuplicatePromptAttribute
//! [ACH1009]: DiagnosticCode::DuplicateValidateAttribute
//! [ACH1010]: DiagnosticCode::ChoicesOnNonChoicePrompt
//! [ACH1011]: DiagnosticCode::MissingChoicesForSelect
//! [ACH1012]: DiagnosticCode::MissingChoicesForMultiselect
//! [ACH1013]: DiagnosticCode::EmptyChoicesList
//! [ACH1014]: DiagnosticCode::DuplicateChoice
//! [ACH1015]: DiagnosticCode::NonStringChoice
//! [ACH1016]: DiagnosticCode::DefaultTypeMismatch
//! [ACH1017]: DiagnosticCode::SelectDefaultNotInChoices
//! [ACH1018]: DiagnosticCode::MultiselectDefaultMustBeArray
//! [ACH1019]: DiagnosticCode::MultiselectDefaultContainsUnknownChoice
//! [ACH1020]: DiagnosticCode::RequiredFalseWithNoDefault
//! [ACH1021]: DiagnosticCode::DuplicateValidateBlock
//! [ACH1022]: DiagnosticCode::InvalidBlueprintVersion
//! [ACH1023]: DiagnosticCode::InvalidMinimumAchitekVersion
//! [ACH2000]: DiagnosticCode::UnknownDependencyReference
//! [ACH2001]: DiagnosticCode::SelfDependency
//! [ACH2002]: DiagnosticCode::DependencyCycle
//! [ACH2003]: DiagnosticCode::DependencyTypeMismatch
//! [ACH2004]: DiagnosticCode::ContainsOnNonMultiselectPrompt
//! [ACH2005]: DiagnosticCode::ContainsUnknownChoice
//! [ACH3000]: DiagnosticCode::StringValidationOnNonStringPrompt
//! [ACH3001]: DiagnosticCode::SelectionValidationOnNonMultiselectPrompt
//! [ACH3002]: DiagnosticCode::InvalidLengthBounds
//! [ACH3003]: DiagnosticCode::InvalidSelectionBounds
//! [ACH3004]: DiagnosticCode::InvalidRegex

/// A user-facing issue found in Achitekfile source.
///
/// Diagnostics carry stable machine-readable metadata that downstream tools can
/// map into their own reporting formats. For example, `achitek-ls` can convert
/// this type into an LSP diagnostic without defining its own Achitekfile
/// diagnostic codes.
///
/// # Examples
///
/// ```
/// let source = r#"
/// blueprint {
///   version = "1.0.0"
///   name = "web-app"
/// }
///
/// prompt "project_name" {
///   help = "Project name"
/// }
/// "#;
///
/// let analysis = achitekfile::analyze(source)?;
/// let diagnostic = &analysis.diagnostics()[0];
///
/// assert_eq!(diagnostic.code(), achitekfile::DiagnosticCode::MissingPromptType);
/// assert_eq!(diagnostic.severity(), achitekfile::Severity::Error);
/// assert_eq!(diagnostic.kind(), achitekfile::DiagnosticKind::Semantic);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagnostic {
    /// Stable identifier for this class of diagnostic.
    code: DiagnosticCode,
    // /// Broad category that produced the diagnostic.
    // kind: DiagnosticKind,
    /// How strongly tooling should surface the diagnostic.
    severity: Severity,
    /// Informational message about the diagnostic.
    message: String,
    /// Help message to assist in remediating the diagnostic.
    help: Option<String>,
    /// The source span where something appears in the achitekfile
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

/// Broad category for an Achitekfile diagnostic.
///
/// The kind describes which analysis layer produced a diagnostic. It is useful
/// for grouping diagnostics in docs and tests, while [`DiagnosticCode`] remains
/// the stable identifier for a specific violation.
///
/// See [`Diagnostic`] for an example of reading a diagnostic kind from analysis
/// output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagnosticKind {
    /// A syntax or parse violation in the source text.
    Syntax,
    /// A semantic violation in syntactically valid Achitekfile source.
    Semantic,
    /// A dependency graph violation between prompt declarations.
    Dependency,
    /// A validation rule violation on a prompt declaration.
    Validation,
}

/// Stable identifiers for Achitekfile diagnostics.
///
/// Codes are part of the public diagnostic contract for downstream tools. Once
/// released, a code should keep the same meaning. Prefer adding a new code over
/// reusing or renumbering an existing one.
///
/// See [`Diagnostic`] for an example of matching on a stable diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagnosticCode {
    /// `ACH0000`: the file does not contain the required `blueprint` block.
    MissingBlueprintBlock,
    /// `ACH0001`: the file contains more than one `blueprint` block.
    MultipleBlueprintBlocks,
    /// `ACH0002`: a `prompt` block appears before the required `blueprint` block.
    PromptBeforeBlueprint,
    /// `ACH0003`: an unsupported item appears at the top level of the file.
    UnknownTopLevelItem,
    /// `ACH0004`: a `blueprint` block contains an unsupported attribute.
    UnknownBlueprintAttribute,
    /// `ACH0005`: a `prompt` block contains an unsupported attribute.
    UnknownPromptAttribute,
    /// `ACH0006`: a `validate` block contains an unsupported attribute.
    UnknownValidateAttribute,
    /// `ACH0007`: a `type` attribute uses an unsupported prompt type.
    UnknownPromptType,
    /// `ACH0008`: a boolean value is not `true` or `false`.
    InvalidBooleanLiteral,
    /// `ACH0009`: a string literal is missing its closing quote.
    UnterminatedString,
    /// `ACH0010`: a string literal contains an unsupported escape sequence.
    InvalidEscapeSequence,
    /// `ACH0011`: a `depends_on` attribute contains an invalid dependency expression.
    InvalidDependencyExpression,
    /// `ACH0012`: a dependency method call uses an unsupported method name.
    UnknownDependencyMethod,
    /// `ACH0013`: an identifier does not match Achitekfile identifier syntax.
    InvalidIdentifier,
    /// `ACH0014`: an integer literal does not match Achitekfile integer syntax.
    InvalidInteger,
    /// `ACH0015`: an array literal is malformed.
    MalformedArray,
    /// `ACH0016`: a `prompt` block is missing its required string name.
    MissingPromptName,
    /// `ACH0017`: an attribute is missing the value after `=`.
    MissingAttributeValue,
    /// `ACH1000`: the `blueprint` block is missing the required `version` attribute.
    MissingBlueprintVersion,
    /// `ACH1001`: the `blueprint` block is missing the required `name` attribute.
    MissingBlueprintName,
    /// `ACH1002`: the `blueprint.name` attribute is empty.
    EmptyBlueprintName,
    /// `ACH1003`: the `blueprint.version` attribute is empty.
    EmptyBlueprintVersion,
    /// `ACH1004`: a `blueprint` block contains the same attribute more than once.
    DuplicateBlueprintAttribute,
    /// `ACH1005`: a `prompt` block is missing the required `type` attribute.
    MissingPromptType,
    /// `ACH1006`: a prompt name is empty.
    EmptyPromptName,
    /// `ACH1007`: more than one prompt uses the same name.
    DuplicatePromptName,
    /// `ACH1008`: a `prompt` block contains the same attribute more than once.
    DuplicatePromptAttribute,
    /// `ACH1009`: a `validate` block contains the same attribute more than once.
    DuplicateValidateAttribute,
    /// `ACH1010`: a non-choice prompt declares `choices`.
    ChoicesOnNonChoicePrompt,
    /// `ACH1011`: a `select` prompt has no choices.
    MissingChoicesForSelect,
    /// `ACH1012`: a `multiselect` prompt has no choices.
    MissingChoicesForMultiselect,
    /// `ACH1013`: a `choices` array is empty.
    EmptyChoicesList,
    /// `ACH1014`: a `choices` array contains the same choice more than once.
    DuplicateChoice,
    /// `ACH1015`: a `choices` array contains a non-string value.
    NonStringChoice,
    /// `ACH1016`: a prompt default does not match the prompt type.
    DefaultTypeMismatch,
    /// `ACH1017`: a `select` default is not one of the prompt choices.
    SelectDefaultNotInChoices,
    /// `ACH1018`: a `multiselect` default is not an array.
    MultiselectDefaultMustBeArray,
    /// `ACH1019`: a `multiselect` default contains a value not listed in choices.
    MultiselectDefaultContainsUnknownChoice,
    /// `ACH1020`: a prompt explicitly sets `required = false` without a default.
    RequiredFalseWithNoDefault,
    /// `ACH1021`: a prompt contains more than one `validate` block.
    DuplicateValidateBlock,
    /// `ACH1022`: the `blueprint.version` attribute is not a valid version.
    InvalidBlueprintVersion,
    /// `ACH1023`: the `blueprint.min_achitek_version` attribute is not a valid version.
    InvalidMinimumAchitekVersion,
    /// `ACH2000`: a dependency references a prompt that does not exist.
    UnknownDependencyReference,
    /// `ACH2001`: a prompt depends on itself.
    SelfDependency,
    /// `ACH2002`: prompt dependencies contain a cycle.
    DependencyCycle,
    /// `ACH2003`: a dependency expression compares incompatible value types.
    DependencyTypeMismatch,
    /// `ACH2004`: a dependency uses `contains` on a prompt that is not `multiselect`.
    ContainsOnNonMultiselectPrompt,
    /// `ACH2005`: a dependency `contains` argument is not one of the referenced prompt choices.
    ContainsUnknownChoice,
    /// `ACH3000`: string validation is used on a non-string prompt.
    StringValidationOnNonStringPrompt,
    /// `ACH3001`: selection-count validation is used on a non-`multiselect` prompt.
    SelectionValidationOnNonMultiselectPrompt,
    /// `ACH3002`: string length validation bounds are invalid.
    InvalidLengthBounds,
    /// `ACH3003`: selection-count validation bounds are invalid.
    InvalidSelectionBounds,
    /// `ACH3004`: a `regex` validation rule is not a valid regular expression.
    InvalidRegex,
}
impl DiagnosticCode {
    /// Returns the broad diagnostic category for this code.
    pub fn kind(&self) -> DiagnosticKind {
        match self {
            Self::MissingBlueprintBlock
            | Self::MultipleBlueprintBlocks
            | Self::PromptBeforeBlueprint
            | Self::UnknownTopLevelItem
            | Self::UnknownBlueprintAttribute
            | Self::UnknownPromptAttribute
            | Self::UnknownValidateAttribute
            | Self::UnknownPromptType
            | Self::InvalidBooleanLiteral
            | Self::UnterminatedString
            | Self::InvalidEscapeSequence
            | Self::InvalidDependencyExpression
            | Self::UnknownDependencyMethod
            | Self::InvalidIdentifier
            | Self::InvalidInteger
            | Self::MalformedArray
            | Self::MissingPromptName
            | Self::MissingAttributeValue => DiagnosticKind::Syntax,
            Self::MissingBlueprintVersion
            | Self::MissingBlueprintName
            | Self::EmptyBlueprintName
            | Self::EmptyBlueprintVersion
            | Self::DuplicateBlueprintAttribute
            | Self::MissingPromptType
            | Self::EmptyPromptName
            | Self::DuplicatePromptName
            | Self::DuplicatePromptAttribute
            | Self::DuplicateValidateAttribute
            | Self::ChoicesOnNonChoicePrompt
            | Self::MissingChoicesForSelect
            | Self::MissingChoicesForMultiselect
            | Self::EmptyChoicesList
            | Self::DuplicateChoice
            | Self::NonStringChoice
            | Self::DefaultTypeMismatch
            | Self::SelectDefaultNotInChoices
            | Self::MultiselectDefaultMustBeArray
            | Self::MultiselectDefaultContainsUnknownChoice
            | Self::RequiredFalseWithNoDefault
            | Self::DuplicateValidateBlock
            | Self::InvalidBlueprintVersion
            | Self::InvalidMinimumAchitekVersion => DiagnosticKind::Semantic,
            Self::UnknownDependencyReference
            | Self::SelfDependency
            | Self::DependencyCycle
            | Self::DependencyTypeMismatch
            | Self::ContainsOnNonMultiselectPrompt
            | Self::ContainsUnknownChoice => DiagnosticKind::Dependency,
            Self::StringValidationOnNonStringPrompt
            | Self::SelectionValidationOnNonMultiselectPrompt
            | Self::InvalidLengthBounds
            | Self::InvalidSelectionBounds
            | Self::InvalidRegex => DiagnosticKind::Validation,
        }
    }
    /// Returns the severity of the diagnostic code.
    pub fn severity(&self) -> Severity {
        match self {
            Self::DuplicateChoice => Severity::Warning,
            Self::RequiredFalseWithNoDefault => Severity::Hint,
            _ => Severity::Error,
        }
    }
    /// Returns the stable machine-readable code.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingBlueprintBlock => "ACH0000",
            Self::MultipleBlueprintBlocks => "ACH0001",
            Self::PromptBeforeBlueprint => "ACH0002",
            Self::UnknownTopLevelItem => "ACH0003",
            Self::UnknownBlueprintAttribute => "ACH0004",
            Self::UnknownPromptAttribute => "ACH0005",
            Self::UnknownValidateAttribute => "ACH0006",
            Self::UnknownPromptType => "ACH0007",
            Self::InvalidBooleanLiteral => "ACH0008",
            Self::UnterminatedString => "ACH0009",
            Self::InvalidEscapeSequence => "ACH0010",
            Self::InvalidDependencyExpression => "ACH0011",
            Self::UnknownDependencyMethod => "ACH0012",
            Self::InvalidIdentifier => "ACH0013",
            Self::InvalidInteger => "ACH0014",
            Self::MalformedArray => "ACH0015",
            Self::MissingPromptName => "ACH0016",
            Self::MissingAttributeValue => "ACH0017",
            Self::MissingBlueprintVersion => "ACH1000",
            Self::MissingBlueprintName => "ACH1001",
            Self::EmptyBlueprintName => "ACH1002",
            Self::EmptyBlueprintVersion => "ACH1003",
            Self::DuplicateBlueprintAttribute => "ACH1004",
            Self::MissingPromptType => "ACH1005",
            Self::EmptyPromptName => "ACH1006",
            Self::DuplicatePromptName => "ACH1007",
            Self::DuplicatePromptAttribute => "ACH1008",
            Self::DuplicateValidateAttribute => "ACH1009",
            Self::ChoicesOnNonChoicePrompt => "ACH1010",
            Self::MissingChoicesForSelect => "ACH1011",
            Self::MissingChoicesForMultiselect => "ACH1012",
            Self::EmptyChoicesList => "ACH1013",
            Self::DuplicateChoice => "ACH1014",
            Self::NonStringChoice => "ACH1015",
            Self::DefaultTypeMismatch => "ACH1016",
            Self::SelectDefaultNotInChoices => "ACH1017",
            Self::MultiselectDefaultMustBeArray => "ACH1018",
            Self::MultiselectDefaultContainsUnknownChoice => "ACH1019",
            Self::RequiredFalseWithNoDefault => "ACH1020",
            Self::DuplicateValidateBlock => "ACH1021",
            Self::InvalidBlueprintVersion => "ACH1022",
            Self::InvalidMinimumAchitekVersion => "ACH1023",
            Self::UnknownDependencyReference => "ACH2000",
            Self::SelfDependency => "ACH2001",
            Self::DependencyCycle => "ACH2002",
            Self::DependencyTypeMismatch => "ACH2003",
            Self::ContainsOnNonMultiselectPrompt => "ACH2004",
            Self::ContainsUnknownChoice => "ACH2005",
            Self::StringValidationOnNonStringPrompt => "ACH3000",
            Self::SelectionValidationOnNonMultiselectPrompt => "ACH3001",
            Self::InvalidLengthBounds => "ACH3002",
            Self::InvalidSelectionBounds => "ACH3003",
            Self::InvalidRegex => "ACH3004",
        }
    }

    /// Returns the default message for this diagnostic code.
    pub fn message(&self) -> &'static str {
        match self {
            Self::MissingBlueprintBlock => "missing blueprint block",
            Self::MultipleBlueprintBlocks => "multiple blueprint blocks",
            Self::PromptBeforeBlueprint => "prompt block appears before blueprint block",
            Self::UnknownTopLevelItem => "unknown top-level item",
            Self::UnknownBlueprintAttribute => "unknown blueprint attribute",
            Self::UnknownPromptAttribute => "unknown prompt attribute",
            Self::UnknownValidateAttribute => "unknown validate attribute",
            Self::UnknownPromptType => "unknown prompt type",
            Self::InvalidBooleanLiteral => "invalid boolean literal",
            Self::UnterminatedString => "unterminated string literal",
            Self::InvalidEscapeSequence => "invalid escape sequence",
            Self::InvalidDependencyExpression => "invalid dependency expression",
            Self::UnknownDependencyMethod => "unknown dependency method",
            Self::InvalidIdentifier => "invalid identifier",
            Self::InvalidInteger => "invalid integer literal",
            Self::MalformedArray => "malformed array literal",
            Self::MissingPromptName => "missing prompt name",
            Self::MissingAttributeValue => "missing attribute value",
            Self::MissingBlueprintVersion => "missing blueprint version",
            Self::MissingBlueprintName => "missing blueprint name",
            Self::EmptyBlueprintName => "empty blueprint name",
            Self::EmptyBlueprintVersion => "empty blueprint version",
            Self::DuplicateBlueprintAttribute => "duplicate blueprint attribute",
            Self::MissingPromptType => "missing prompt type",
            Self::EmptyPromptName => "empty prompt name",
            Self::DuplicatePromptName => "duplicate prompt name",
            Self::DuplicatePromptAttribute => "duplicate prompt attribute",
            Self::DuplicateValidateAttribute => "duplicate validate attribute",
            Self::ChoicesOnNonChoicePrompt => "choices on non-choice prompt",
            Self::MissingChoicesForSelect => "missing choices for select prompt",
            Self::MissingChoicesForMultiselect => "missing choices for multiselect prompt",
            Self::EmptyChoicesList => "empty choices list",
            Self::DuplicateChoice => "duplicate choice",
            Self::NonStringChoice => "non-string choice",
            Self::DefaultTypeMismatch => "default type mismatch",
            Self::SelectDefaultNotInChoices => "select default is not in choices",
            Self::MultiselectDefaultMustBeArray => "multiselect default must be an array",
            Self::MultiselectDefaultContainsUnknownChoice => {
                "multiselect default contains unknown choice"
            }
            Self::RequiredFalseWithNoDefault => "required false with no default",
            Self::DuplicateValidateBlock => "duplicate validate block",
            Self::InvalidBlueprintVersion => "invalid blueprint version",
            Self::InvalidMinimumAchitekVersion => "invalid minimum Achitek version",
            Self::UnknownDependencyReference => "dependency references unknown prompt",
            Self::SelfDependency => "dependency references itself",
            Self::DependencyCycle => "dependency cycle",
            Self::DependencyTypeMismatch => "dependency type mismatch",
            Self::ContainsOnNonMultiselectPrompt => "contains on non-multiselect prompt",
            Self::ContainsUnknownChoice => "contains unknown choice",
            Self::StringValidationOnNonStringPrompt => "string validation on non-string prompt",
            Self::SelectionValidationOnNonMultiselectPrompt => {
                "selection validation on non-multiselect prompt"
            }
            Self::InvalidLengthBounds => "invalid length bounds",
            Self::InvalidSelectionBounds => "invalid selection bounds",
            Self::InvalidRegex => "invalid regex",
        }
    }

    /// Returns default help text for this diagnostic code.
    pub fn help(&self) -> Option<&'static str> {
        match self {
            Self::MissingBlueprintBlock => Some("Start the file with a `blueprint { ... }` block."),
            Self::MultipleBlueprintBlocks => {
                Some("Keep exactly one `blueprint` block in each Achitekfile.")
            }
            Self::PromptBeforeBlueprint => {
                Some("Move the `blueprint` block before all `prompt` blocks.")
            }
            Self::UnknownTopLevelItem => {
                Some("Only `blueprint` and `prompt` blocks are valid at the top level.")
            }
            Self::UnknownBlueprintAttribute => Some(
                "Use one of `version`, `name`, `description`, `author`, or `min_achitek_version`.",
            ),
            Self::UnknownPromptAttribute => Some(
                "Use one of `type`, `help`, `choices`, `default`, `required`, `depends_on`, or `validate`.",
            ),
            Self::UnknownValidateAttribute => Some(
                "Use one of `regex`, `min_length`, `max_length`, `min_selections`, or `max_selections`.",
            ),
            Self::UnknownPromptType => {
                Some("Use one of `string`, `paragraph`, `bool`, `select`, or `multiselect`.")
            }
            Self::InvalidBooleanLiteral => Some("Use `true` or `false`."),
            Self::UnterminatedString => Some("Close the string with `\"`."),
            Self::InvalidEscapeSequence => {
                Some("Supported escapes are `\\n`, `\\t`, `\\r`, `\\\"`, and `\\\\`.")
            }
            Self::InvalidDependencyExpression => Some(
                "Use a prompt reference, comparison, `contains(...)`, `all(...)`, or `any(...)`.",
            ),
            Self::UnknownDependencyMethod => Some("The only supported method is `contains`."),
            Self::InvalidIdentifier => Some(
                "Identifiers must start with a letter and contain only letters, digits, or `_`.",
            ),
            Self::InvalidInteger => Some("Use a non-negative integer such as `1` or `42`."),
            Self::MalformedArray => Some("Use `[value, value]` with comma-separated values."),
            Self::MissingPromptName => Some("Write the prompt name as `prompt \"name\" { ... }`."),
            Self::MissingAttributeValue => Some("Add a value after `=`, or remove the attribute."),
            Self::MissingBlueprintVersion => {
                Some("Add a `version = \"...\"` attribute to the `blueprint` block.")
            }
            Self::MissingBlueprintName => {
                Some("Add a `name = \"...\"` attribute to the `blueprint` block.")
            }
            Self::EmptyBlueprintName => Some("Use a non-empty blueprint `name` value."),
            Self::EmptyBlueprintVersion => Some("Use a non-empty blueprint `version` value."),
            Self::DuplicateBlueprintAttribute => {
                Some("Keep one value for each `blueprint` attribute.")
            }
            Self::MissingPromptType => Some("Add a `type = ...` attribute to the prompt block."),
            Self::EmptyPromptName => Some("Use a non-empty prompt name."),
            Self::DuplicatePromptName => Some("Give each prompt a unique name."),
            Self::DuplicatePromptAttribute => {
                Some("Keep one value for each prompt attribute in a prompt block.")
            }
            Self::DuplicateValidateAttribute => {
                Some("Keep one value for each validation attribute in a `validate` block.")
            }
            Self::ChoicesOnNonChoicePrompt => {
                Some("Use `choices` only with `select` or `multiselect` prompts.")
            }
            Self::MissingChoicesForSelect => {
                Some("Add a non-empty `choices = [...]` array to the `select` prompt.")
            }
            Self::MissingChoicesForMultiselect => {
                Some("Add a non-empty `choices = [...]` array to the `multiselect` prompt.")
            }
            Self::EmptyChoicesList => Some("Add at least one string choice."),
            Self::DuplicateChoice => Some("Remove the duplicate choice value."),
            Self::NonStringChoice => Some("Use string literals for prompt choices."),
            Self::DefaultTypeMismatch => Some("Use a default value that matches the prompt type."),
            Self::SelectDefaultNotInChoices => {
                Some("Set the default to one of the values in `choices`.")
            }
            Self::MultiselectDefaultMustBeArray => {
                Some("Use an array default such as `default = [\"one\"]`.")
            }
            Self::MultiselectDefaultContainsUnknownChoice => {
                Some("Every default value must also appear in `choices`.")
            }
            Self::RequiredFalseWithNoDefault => {
                Some("Remove `required = false` or provide a useful `default` value.")
            }
            Self::DuplicateValidateBlock => {
                Some("Merge validation rules into a single `validate { ... }` block.")
            }
            Self::InvalidBlueprintVersion => {
                Some("Use three numeric version components such as `1.0.0`.")
            }
            Self::InvalidMinimumAchitekVersion => {
                Some("Use three numeric minimum Achitek version components such as `1.0.0`.")
            }
            Self::UnknownDependencyReference => {
                Some("Reference the name of another prompt declared in this file.")
            }
            Self::SelfDependency => Some("A prompt cannot depend on itself."),
            Self::DependencyCycle => Some("Remove or rewrite one dependency to break the cycle."),
            Self::DependencyTypeMismatch => {
                Some("Compare dependency values with values that match the referenced prompt type.")
            }
            Self::ContainsOnNonMultiselectPrompt => {
                Some("Use `.contains(...)` only with `multiselect` prompt dependencies.")
            }
            Self::ContainsUnknownChoice => {
                Some("Use a `.contains(...)` value that appears in the referenced prompt choices.")
            }
            Self::StringValidationOnNonStringPrompt => Some(
                "Use string length or regex validation only on `string` or `paragraph` prompts.",
            ),
            Self::SelectionValidationOnNonMultiselectPrompt => {
                Some("Use selection-count validation only on `multiselect` prompts.")
            }
            Self::InvalidLengthBounds => {
                Some("Ensure `min_length` is less than or equal to `max_length`.")
            }
            Self::InvalidSelectionBounds => {
                Some("Ensure `min_selections` is less than or equal to `max_selections`.")
            }
            Self::InvalidRegex => Some("Use a regex pattern that can be compiled by Achitek."),
        }
    }
}

/// Severity level for an Achitekfile diagnostic.
///
/// Severity indicates how tools should present a diagnostic. Errors describe
/// invalid source that should prevent normal execution. Warnings describe
/// suspicious but still analyzable source. Hints provide low-priority guidance.
///
/// See [`Diagnostic`] for an example of reading diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Severity {
    /// Invalid source that should prevent normal execution.
    Error,
    /// Suspicious source that can still be analyzed.
    Warning,
    /// Low-priority guidance.
    Hint,
}

/// A zero-based byte position in Achitekfile source text.
///
/// `line` and `byte` use Tree-sitter's native coordinate system: the line is
/// zero-based and `byte` is the zero-based UTF-8 byte offset from the beginning
/// of that line.
///
/// This type is independent of [LSP] positions. Language-server consumers
/// should convert `byte` into the negotiated LSP position encoding before
/// publishing diagnostics or other ranges to an editor.
///
/// [LSP]: https://microsoft.github.io/language-server-protocol/
///
/// # Examples
///
/// ```
/// let position = achitekfile::TextPosition { line: 3, byte: 12 };
///
/// assert_eq!(position.line, 3);
/// assert_eq!(position.byte, 12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextPosition {
    /// Zero-based line number.
    pub line: usize,

    /// Zero-based UTF-8 byte offset within the line.
    pub byte: usize,
}

/// A byte range in Achitekfile source text.
///
/// The range starts at `start` and ends at `end`, both expressed as zero-based
/// line plus UTF-8 byte offset positions. Consumers can use this to highlight
/// diagnostics, symbols, prompt names, attributes, and other source elements
/// after converting into their presentation protocol's expected position
/// encoding.
///
/// # Examples
///
/// ```
/// let source = r#"
/// prompt "project_name" {
///   type = string
/// }
/// "#;
///
/// let analysis = achitekfile::analyze(source)?;
/// let range = analysis.diagnostics()[0].range();
///
/// assert!(range.start <= range.end);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextRange {
    /// Start position of the range.
    pub start: TextPosition,

    /// End position of the range.
    pub end: TextPosition,
}
