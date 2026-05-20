//! Domain model types for Tera template source.
//!
//! This module holds the recovering semantic representation that sits between
//! the raw Tree-sitter syntax tree and consumers such as language servers. The
//! model is intentionally factual: it records which Tera constructs were found
//! and where they appeared, while diagnostics decide whether those constructs
//! are valid for a particular context.

pub use achitek_source::Spanned;

/// Recovering semantic representation of one Tera template.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TeraFile {
    dependencies: Vec<Spanned<TemplateDependency>>,
    macros: Vec<Spanned<Macro>>,
    bindings: Vec<Spanned<Binding>>,
    variable_references: Vec<Spanned<VariableReference>>,
    filters: Vec<Spanned<NamedReference>>,
    tests: Vec<Spanned<NamedReference>>,
    functions: Vec<Spanned<NamedReference>>,
    macro_calls: Vec<Spanned<MacroCall>>,
}

impl TeraFile {
    /// Creates a recovered Tera model from parsed semantic facts.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        dependencies: Vec<Spanned<TemplateDependency>>,
        macros: Vec<Spanned<Macro>>,
        bindings: Vec<Spanned<Binding>>,
        variable_references: Vec<Spanned<VariableReference>>,
        filters: Vec<Spanned<NamedReference>>,
        tests: Vec<Spanned<NamedReference>>,
        functions: Vec<Spanned<NamedReference>>,
        macro_calls: Vec<Spanned<MacroCall>>,
    ) -> Self {
        Self {
            dependencies,
            macros,
            bindings,
            variable_references,
            filters,
            tests,
            functions,
            macro_calls,
        }
    }

    /// Returns template dependencies declared by `extends`, `include`, or
    /// `import`.
    pub fn dependencies(&self) -> &[Spanned<TemplateDependency>] {
        &self.dependencies
    }

    /// Returns macro definitions in source order.
    pub fn macros(&self) -> &[Spanned<Macro>] {
        &self.macros
    }

    /// Returns local bindings recovered from assignments, loops, macros, and
    /// imports.
    pub fn bindings(&self) -> &[Spanned<Binding>] {
        &self.bindings
    }

    /// Returns variable references recovered from expressions.
    pub fn variable_references(&self) -> &[Spanned<VariableReference>] {
        &self.variable_references
    }

    /// Returns filter names used by expressions or filter sections.
    pub fn filters(&self) -> &[Spanned<NamedReference>] {
        &self.filters
    }

    /// Returns test names used by `is` expressions.
    pub fn tests(&self) -> &[Spanned<NamedReference>] {
        &self.tests
    }

    /// Returns global function calls.
    pub fn functions(&self) -> &[Spanned<NamedReference>] {
        &self.functions
    }

    /// Returns namespaced macro calls such as `self::name()` or
    /// `forms::field()`.
    pub fn macro_calls(&self) -> &[Spanned<MacroCall>] {
        &self.macro_calls
    }
}

/// A dependency on another Tera template.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TemplateDependency {
    /// The construct that declared the dependency.
    pub kind: TemplateDependencyKind,
    /// The static path or fallback path list declared by the dependency.
    pub path: TemplatePath,
}

/// The construct that declared a template dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TemplateDependencyKind {
    /// `{% extends "base.html" %}`.
    Extends,
    /// `{% include "item.html" %}`.
    Include {
        /// Whether the include uses `ignore missing`.
        ignore_missing: bool,
    },
    /// `{% import "macros.html" as forms %}`.
    Import {
        /// Namespace used to access imported macros.
        namespace: Option<String>,
    },
}

/// Static template path information.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TemplatePath {
    /// One static string path.
    Single(String),
    /// A static fallback list used by `include`.
    Choice(Vec<String>),
}

/// A Tera macro definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Macro {
    /// Macro name.
    pub name: String,
    /// Macro parameters.
    pub parameters: Vec<Spanned<MacroParameter>>,
}

/// One macro parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MacroParameter {
    /// Parameter name.
    pub name: String,
    /// Whether the parameter declares a default value.
    pub has_default: bool,
}

/// A variable binding introduced by Tera source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Binding {
    /// Bound identifier.
    pub name: String,
    /// Construct that introduced the binding.
    pub kind: BindingKind,
}

/// The construct that introduced a binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BindingKind {
    /// `{% set name = value %}`.
    Set,
    /// `{% set_global name = value %}`.
    SetGlobal,
    /// A loop variable introduced by `{% for item in items %}`.
    ForVariable,
    /// The built-in `loop` variable available inside loops.
    LoopVariable,
    /// A macro parameter.
    MacroParameter,
    /// A namespace introduced by an `import` statement.
    ImportNamespace,
}

/// A variable reference recovered from an expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VariableReference {
    /// Variable path text as written, such as `product.name`.
    pub path: String,
    /// Root identifier for the reference, such as `product`.
    pub root: String,
}

/// A named construct reference, such as a filter, test, or global function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NamedReference {
    /// Referenced name.
    pub name: String,
}

/// A namespaced macro call.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MacroCall {
    /// Namespace used for the call, such as `self` or an imported namespace.
    pub namespace: String,
    /// Macro name.
    pub name: String,
    /// Keyword argument names passed to the macro.
    pub arguments: Vec<Spanned<NamedReference>>,
}
