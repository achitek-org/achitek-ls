//! Domain model types for Achitekfile source.
//!
//! This module holds the Achitekfile representation that sits between the raw
//! Tree-sitter syntax tree and consumers.
//!
//! The parser layer answers "what syntax tree did Tree-sitter produce?" while
//! this layer should answer "what Achitekfile concepts were found in the
//! source?"
//!
//! There are two model families because Achitekfile consumers have different
//! tolerance for invalid source:
//!
//! - [`AchitekFile`] is the recovering model. It is designed for editor and
//!   diagnostic workflows where the source may be incomplete, malformed, or
//!   mid-edit. Fields that may be missing are represented with [`Option`], and
//!   recovered values can be wrapped in [`Spanned`] so tools can connect model
//!   values back to source ranges. Language servers, formatters, documentation
//!   generators, and rich CLI validation should generally start here.
//! - [`ValidAchitekFile`] is the strict model. It represents an Achitekfile
//!   after validation has proven that required structure exists and prompt data
//!   is complete enough to execute. Runtime consumers such as the Achitek CLI
//!   should reach for this model when scaffolding projects, evaluating
//!   dependencies, or applying prompt validation rules.
//!
//! The split keeps editor tooling useful while the user is still typing without
//! weakening the runtime contract. Invalid source can still produce a useful
//! [`AchitekFile`] plus diagnostics, while execution can require a
//! [`ValidAchitekFile`] and avoid repeatedly checking whether required values
//! are present.
//!
//! Keep Tree-sitter implementation details out of these types. Model values
//! should describe Achitek concepts such as blueprints, prompts, prompt types,
//! defaults, validation rules, and dependency expressions. When a value needs
//! to point back into source text, prefer crate-owned range types such as
//! [`TextRange`] instead of exposing Tree-sitter nodes directly.

use super::{
    TextRange,
    sort::{Graph, SortError, sort_graph},
};
use std::{collections::HashMap, vec};

/// A value paired with the source range that produced it.
///
/// Spans let editor-facing consumers connect recovered model values back to the
/// original source text without exposing Tree-sitter nodes.
///
/// # Examples
///
/// ```
/// let source = r#"
/// blueprint {
///   version = "1.0.0"
///   name = "web-app"
/// }
/// "#;
///
/// let analysis = achitekfile::analyze(source)?;
/// let version = analysis
///     .file()
///     .blueprint()
///     .version
///     .as_ref()
///     .map(|spanned| spanned.as_ref().as_str());
///
/// assert_eq!(version, Some("1.0.0"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Spanned<T> {
    /// Recovered model value.
    pub value: T,
    /// Source range that produced the value.
    pub range: TextRange,
}

impl<T> AsRef<T> for Spanned<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T> AsMut<T> for Spanned<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

/// Recovering blueprint metadata.
///
/// The recovering model keeps fields optional because invalid source may omit
/// required blueprint attributes. A later validation step can turn this into a
/// [`ValidBlueprint`] once required fields are known to be present.
///
/// See [`AchitekFile`] for an example of reading recovered blueprint metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blueprint {
    /// Source range for the recovered `blueprint { ... }` block.
    ///
    /// This is `None` when no blueprint block was recovered. Diagnostics for
    /// missing blueprint attributes can use this range to point at the block
    /// that should contain the missing field.
    pub range: Option<TextRange>,
    /// Achitekfile format version declared by the blueprint.
    pub version: Option<Spanned<String>>,
    /// Blueprint name.
    pub name: Option<Spanned<String>>,
    /// Optional blueprint description.
    pub description: Option<Spanned<String>>,
    /// Optional blueprint author.
    pub author: Option<Spanned<String>>,
    /// Optional minimum Achitek version required by the blueprint.
    pub min_achitek_version: Option<Spanned<String>>,
}

/// Semantic representation of an Achitekfile.
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
///   type = string
/// }
/// "#;
///
/// let analysis = achitekfile::analyze(source)?;
/// let file = analysis.file();
///
/// assert_eq!(file.blueprint().name.as_ref().map(|name| name.value.as_str()), Some("web-app"));
/// assert_eq!(file.prompts()[0].value.name, "project_name");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AchitekFile {
    blueprint: Blueprint,
    prompts: Vec<Spanned<Prompt>>,
}

impl AchitekFile {
    /// Creates a recovering Achitekfile model from parsed parts.
    ///
    /// See [`AchitekFile`] for a parsing-oriented example.
    pub fn new(blueprint: Blueprint, prompts: Vec<Spanned<Prompt>>) -> Self {
        Self { blueprint, prompts }
    }
    /// Returns recovered blueprint metadata.
    ///
    /// See [`AchitekFile`] for a complete example.
    pub fn blueprint(&self) -> &Blueprint {
        &self.blueprint
    }
    /// Returns recovered prompts in source order.
    ///
    /// See [`AchitekFile`] for a complete example.
    pub fn prompts(&self) -> &[Spanned<Prompt>] {
        &self.prompts
    }
}

/// A parsed prompt declaration from an Achitekfile.
///
/// See [`AchitekFile`] for an example that reads recovered prompts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Prompt {
    /// The prompt identifier from `prompt "..."`.
    pub name: String,
    /// The kind of input expected for this prompt.
    ///
    /// This is optional in the recovering model so diagnostics can report a
    /// missing `type` attribute without discarding the rest of the prompt.
    pub prompt_type: Option<PromptType>,
    /// Optional help text shown to a user alongside the prompt.
    pub help: Option<String>,
    /// The allowed choices for `select` and `multiselect` prompts.
    ///
    /// Non-choice prompt types may leave this empty.
    pub choices: Vec<Value>,
    /// Whether the prompt declared a `choices` attribute.
    ///
    /// This lets analysis distinguish an omitted `choices` attribute from an
    /// explicitly empty `choices = []` array.
    pub choices_declared: bool,
    /// The default answer for the prompt, if one was declared.
    pub default: Option<Value>,
    /// Whether the prompt requires an answer.
    ///
    /// `None` means the Achitekfile omitted the `required` attribute and the
    /// caller should apply its own default policy.
    pub required: Option<bool>,
    /// A dependency expression that controls whether this prompt should be
    /// asked.
    pub depends_on: Option<Dependency>,
    /// Validation rules declared in the nested `validate { ... }` block.
    pub validation: Validation,
}

/// The supported prompt input types.
///
/// See [`Prompt`] for the containing model type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PromptType {
    /// A single-line string answer.
    String,
    /// A multi-line string answer.
    Paragraph,
    /// A boolean answer.
    Bool,
    /// A single choice from the prompt's `choices`.
    Select,
    /// Zero or more choices from the prompt's `choices`.
    MultiSelect,
}

/// A literal or identifier value parsed from an Achitekfile.
///
/// See [`Prompt`] for an example of values attached to parsed prompts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    /// A double-quoted string literal with supported escape sequences decoded.
    String(String),
    /// A `true` or `false` literal.
    Bool(bool),
    /// A non-negative integer literal.
    Integer(u64),
    /// An unquoted identifier.
    Identifier(String),
    /// An array of values.
    Array(Vec<Value>),
}

/// A dependency expression from a prompt's `depends_on` attribute.
///
/// Dependencies are both executable conditions and graph edges. For ordering,
/// every variant can reveal the prompt names it references:
///
/// - `database`
/// - `database != "none"`
/// - `features.contains("auth")`
/// - `all(database != "none", features.contains("auth"))`
///
/// See [`ValidAchitekFile::prompts_in`] for an example of using dependencies
/// to order prompts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Dependency {
    /// A direct dependency on another prompt by name, such as `depends_on = database`.
    Reference(String),
    /// A comparison dependency, such as `database != "none"`.
    Comparison {
        /// The prompt name on the left-hand side of the comparison.
        left: String,
        /// The equality operator used by the comparison.
        operator: ComparisonOperator,
        /// The literal value on the right-hand side of the comparison.
        right: Value,
    },
    /// A `contains` dependency, such as `features.contains("auth")`.
    Contains {
        /// The prompt name whose answer is searched.
        receiver: String,
        /// The value expected to be contained in the receiver's answer.
        argument: Value,
    },
    /// A dependency that requires every nested dependency to be true.
    All(Vec<Dependency>),
    /// A dependency that requires at least one nested dependency to be true.
    Any(Vec<Dependency>),
}
impl Dependency {
    fn references(&self) -> Vec<&str> {
        let mut references = Vec::new();
        self.collect_references(&mut references);
        references
    }

    fn collect_references<'a>(&'a self, references: &mut Vec<&'a str>) {
        match self {
            Self::Reference(name) => references.push(name),
            Self::Comparison { left, .. } => references.push(left),
            Self::Contains { receiver, .. } => references.push(receiver),
            Self::All(dependencies) | Self::Any(dependencies) => {
                for dependency in dependencies {
                    dependency.collect_references(references);
                }
            }
        }
    }
}

/// Operators supported by comparison dependencies.
///
/// See [`Dependency`] for the comparison expression that uses this operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ComparisonOperator {
    /// Equality, written as `==`.
    Equal,
    /// Inequality, written as `!=`.
    NotEqual,
}

/// Validation rules for a prompt.
///
/// These fields correspond to attributes inside a `validate { ... }` block.
/// The parser records what the file declares; it does not currently enforce
/// whether a given rule is appropriate for the prompt type.
///
/// See [`Prompt`] and [`ValidPrompt`] for examples of prompts that carry
/// validation rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Validation {
    /// A regular expression that string-like answers must match.
    pub regex: Option<String>,
    /// The minimum accepted string length.
    pub min_length: Option<u64>,
    /// The maximum accepted string length.
    pub max_length: Option<u64>,
    /// The minimum number of selections for a multiselect prompt.
    pub min_selections: Option<u64>,
    /// The maximum number of selections for a multiselect prompt.
    pub max_selections: Option<u64>,
}

/// A valid semantic representation of an Achitekfile.
///
/// This type is intended for runtime consumers that need a file which has
/// already passed syntax and semantic validation. Unlike [`AchitekFile`], it
/// does not expose partial or optional structure for required concepts: a valid
/// file always has a blueprint and every prompt has a complete prompt type.
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
/// prompt "database" {
///   type = select
///   choices = ["postgres", "sqlite"]
/// }
/// "#;
///
/// let file = achitekfile::analyze(source)?.into_valid().map_err(|diagnostics| {
///     let message = diagnostics
///         .into_iter()
///         .map(|diagnostic| diagnostic.message().to_owned())
///         .collect::<Vec<_>>()
///         .join(", ");
///     std::io::Error::new(std::io::ErrorKind::InvalidData, message)
/// })?;
///
/// assert_eq!(file.prompts()[0].prompt_type, achitekfile::model::PromptType::Select);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValidAchitekFile {
    blueprint: ValidBlueprint,
    prompts: Vec<ValidPrompt>,
}

impl ValidAchitekFile {
    /// Creates a valid Achitekfile model from already validated parts.
    ///
    /// See [`ValidAchitekFile`] for a validation-oriented example.
    pub fn new(blueprint: ValidBlueprint, prompts: Vec<ValidPrompt>) -> Self {
        Self { blueprint, prompts }
    }

    /// Returns the validated blueprint metadata.
    ///
    /// See [`ValidAchitekFile`] for a complete example.
    pub fn blueprint(&self) -> &ValidBlueprint {
        &self.blueprint
    }

    /// Returns validated prompts in source order.
    ///
    /// See [`ValidAchitekFile`] for a complete example.
    pub fn prompts(&self) -> &[ValidPrompt] {
        &self.prompts
    }

    /// Returns validated prompts in the requested order.
    ///
    /// # Errors
    ///
    /// Returns [`SortError`] when dependency ordering is requested and
    /// the prompt dependency graph contains a cycle. Use
    /// [`SortError::cycles`] to inspect the cyclic regions.
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
    /// prompt "orm" {
    ///   type = select
    ///   choices = ["sqlx", "diesel"]
    ///   depends_on = database != "sqlite"
    /// }
    ///
    /// prompt "database" {
    ///   type = select
    ///   choices = ["postgres", "sqlite"]
    /// }
    /// "#;
    ///
    /// let file = achitekfile::analyze(source)?.into_valid().map_err(|diagnostics| {
    ///     let message = diagnostics
    ///         .into_iter()
    ///         .map(|diagnostic| diagnostic.message().to_owned())
    ///         .collect::<Vec<_>>()
    ///         .join(", ");
    ///     std::io::Error::new(std::io::ErrorKind::InvalidData, message)
    /// })?;
    /// let ordered_names = file
    ///     .prompts_in(achitekfile::model::PromptOrder::Dependency)?
    ///     .map(|prompt| prompt.name.as_str())
    ///     .collect::<Vec<_>>();
    ///
    /// assert_eq!(ordered_names, ["database", "orm"]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn prompts_in(&self, order: PromptOrder) -> Result<PromptIter<'_>, SortError<String>> {
        match order {
            PromptOrder::Source => Ok(PromptIter::Source(self.prompts.iter())),
            PromptOrder::Dependency => self.prompts_in_dependency_order(),
        }
    }

    fn prompts_in_dependency_order(&self) -> Result<PromptIter<'_>, SortError<String>> {
        let prompt_names = self
            .prompts
            .iter()
            .map(|prompt| prompt.name.clone())
            .collect::<Vec<_>>();
        let edges = self
            .prompts
            .iter()
            .flat_map(|prompt| {
                prompt
                    .depends_on
                    .as_ref()
                    .into_iter()
                    .flat_map(|dependency| dependency.references())
                    .map(|reference| (reference.to_owned(), prompt.name.clone()))
            })
            .collect::<Vec<_>>();
        let graph = Graph {
            nodes: prompt_names,
            edges,
        };
        let sorted_names = sort_graph(&graph)?;
        let prompts_by_name = self
            .prompts
            .iter()
            .map(|prompt| (prompt.name.as_str(), prompt))
            .collect::<HashMap<_, _>>();
        let prompts = sorted_names
            .iter()
            .filter_map(|name| prompts_by_name.get(name.as_str()).copied())
            .collect::<Vec<_>>();

        Ok(PromptIter::Dependency(prompts.into_iter()))
    }
}

/// Ordering strategy for validated prompts.
///
/// See [`ValidAchitekFile::prompts_in`] for an example.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PromptOrder {
    /// Preserve the order prompts appeared in the source file.
    Source,
    /// Return dependency prompts before prompts that depend on them.
    Dependency,
}

/// Iterator over validated prompts.
///
/// See [`ValidAchitekFile::prompts_in`] for an example.
#[derive(Debug, Clone)]
pub enum PromptIter<'a> {
    /// Iterates over prompts in source order without allocating.
    Source(std::slice::Iter<'a, ValidPrompt>),
    /// Iterates over prompts in computed dependency order.
    Dependency(vec::IntoIter<&'a ValidPrompt>),
}

impl<'a> Iterator for PromptIter<'a> {
    type Item = &'a ValidPrompt;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Source(prompts) => prompts.next(),
            Self::Dependency(prompts) => prompts.next(),
        }
    }
}

/// Validated blueprint metadata.
///
/// See [`ValidAchitekFile`] for an example that reads validated blueprint
/// metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValidBlueprint {
    /// Achitekfile format version declared by the blueprint.
    pub version: String,
    /// Blueprint name.
    pub name: String,
    /// Optional blueprint description.
    pub description: Option<String>,
    /// Optional blueprint author.
    pub author: Option<String>,
    /// Optional minimum Achitek version required by the blueprint.
    pub min_achitek_version: Option<String>,
}

/// A validated prompt declaration.
///
/// See [`ValidAchitekFile`] for an example that reads validated prompts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValidPrompt {
    /// The prompt identifier from `prompt "..."`.
    pub name: String,
    /// The kind of input expected for this prompt.
    pub prompt_type: PromptType,
    /// Optional help text shown to a user alongside the prompt.
    pub help: Option<String>,
    /// The allowed choices for `select` and `multiselect` prompts.
    pub choices: Vec<Value>,
    /// The default answer for the prompt, if one was declared.
    pub default: Option<Value>,
    /// Whether the prompt requires an answer.
    pub required: bool,
    /// A dependency expression that controls whether this prompt should be
    /// asked.
    pub depends_on: Option<Dependency>,
    /// Validation rules declared in the nested `validate { ... }` block.
    pub validation: Validation,
}
