# Capabilities

This document captures the language server capabilities that are most relevant
for Achitekfile authoring. It is intentionally opinionated: the goal is not to
implement every possible LSP feature, but to prioritize the ones that make this
DSL easier to write, validate, and maintain.

## Context

Achitekfile is a declarative configuration language for describing interactive
template blueprints. The most valuable editor features are the ones that help
authors:

- write valid blocks and attributes
- understand allowed values and constraints
- navigate references between prompts
- catch mistakes as early as possible
- safely rename identifiers used in dependency expressions and templates

Because the language is declarative and relatively compact, diagnostics,
completion, navigation, and rename are more important than heavyweight IDE
features like call hierarchies or code actions built around complex program
flow.

## Capability Tiers

## Tier 1: Must-Have

These capabilities directly support day-to-day authoring.

- Syntax diagnostics
  Report malformed blocks, missing braces, unexpected tokens, and incomplete
  constructs while the user types.
- Semantic diagnostics
  Validate rules the parser alone cannot catch, such as duplicate prompt names,
  unknown prompt references, invalid attributes, incompatible defaults, and
  inconsistent validation rules.
- Template diagnostics
  Report unknown prompt references in nearby `.tera` template files.
- Document synchronization
  Analyze the in-memory editor buffer on open and change, not just files on
  disk.
- Completion
  Provide completions for block keywords, attribute keys, prompt types, known
  prompt references, and dependency-expression helpers.
- Hover
  Explain block types, attribute keys, prompt types, dependency operators, and
  combinators.
- Document symbols
  Surface top-level structure such as the blueprint block and each prompt block
  so users can quickly navigate large files.

## Tier 2: High-Value

These features are especially relevant to Achitekfile once the core authoring
loop is stable.

- Go to definition
  Jump from a prompt reference inside a dependency expression or `.tera`
  template to the prompt block that defines it.
- Find references
  Show every use of a prompt name across dependency expressions and nearby
  `.tera` templates.
- Rename
  Rename a prompt identifier and update dependency-expression references plus
  matching `.tera` template references.
- Semantic highlighting
  Distinguish prompt names, attribute keys, question types, literals, and
  dependency operators more precisely than syntax highlighting alone.
- Inlay hints
  Show lightweight hints for inferred meaning where useful, such as expected
  attribute value shapes or dependency target names.

## Tier 3: Nice to Have

These are useful, but they should not come before the core authoring
experience.

- Formatting
  Normalize indentation, spacing, trailing commas, and block layout.
- Folding ranges
  Fold blueprint, prompt, and validate blocks.
- Selection ranges
  Expand editor selections by syntax node.
- Code actions
  Offer small fixes such as inserting missing required attributes, replacing
  invalid attribute keys, or removing duplicate attributes.
- Workspace symbols
  Search prompts across open Achitek documents.

## Capability Mapping by Module

- `syntax`
  Enables parsing, source ranges, and syntax diagnostics.
- `analysis`
  Enables semantic diagnostics, navigation, rename logic, hover text, and
  completion data.
- `server`
  Exposes those capabilities over LSP, manages document state, and handles
  cross-file template scans.

## LSP Request Matrix

This table tracks the request-style LSP methods that are relevant to this
language server. Methods we already know we do not want to implement are
intentionally omitted.

### Requests

| LSP method | Tier | Implemented | Note |
| :--- | :---: | :---: | :--- |
| `initialize` | 1 | ✅ | Foundational handshake, `serverInfo`, and capability advertisement |
| `shutdown` | 1 | ✅ | Uses `lsp_server` shutdown handling |
| `textDocument/codeAction` | 3 | ❌ | Useful after diagnostics are stable |
| `textDocument/completion` | 1 | ✅ | Block, attribute, type, reference, and dependency-expression completion |
| `textDocument/definition` | 2 | ✅ | Prompt references and `.tera` template references to prompt definitions |
| `textDocument/documentSymbol` | 1 | ✅ | Outline of blueprint, prompts, and validate blocks |
| `textDocument/foldingRange` | 3 | ✅ | Fold blueprint, prompt, and validate blocks |
| `textDocument/formatting` | 3 | ✅ | Normalize layout and whitespace |
| `textDocument/hover` | 1 | ✅ | Show prompt, attribute, and dependency docs |
| `textDocument/inlayHint` | 2 | ❌ | Optional hints for inferred meaning or expected values |
| `textDocument/prepareRename` | 2 | ✅ | Validate rename targets before rename is applied |
| `textDocument/references` | 2 | ✅ | Find prompt usages across dependency expressions and templates |
| `textDocument/rename` | 2 | ✅ | Rename prompts and update Achitekfile/template references consistently |
| `textDocument/selectionRange` | 3 | ✅ | Syntax-driven selection expansion |
| `textDocument/semanticTokens/full` | 2 | ❌ | Semantic highlighting beyond grammar scopes |
| `workspace/symbol` | 3 | ✅ | Search prompts across open Achitek documents |

### Notifications

| LSP method | Implemented | Note |
| :--- | :---: | :--- |
| `textDocument/didOpen` | ✅ | Stores the editor buffer and publishes document/template diagnostics |
| `textDocument/didChange` | ✅ | Applies full-document sync and republishes document/template diagnostics |
| `textDocument/didClose` | ✅ | Removes the document and clears diagnostics |

## Recommended Implementation Order

1. Add code actions for common diagnostic fixes
2. Add semantic tokens if grammar highlighting is not expressive enough
3. Add inlay hints where they clarify expected value shapes
4. Consider broader workspace indexing once document-local features remain
   stable

## Achitek-Specific Validation Targets

The following checks are especially valuable for this DSL and should live in
`analysis` or template-aware server helpers:

- only one `blueprint` block per file
- required blueprint attributes are present
- prompt names are unique
- prompt attributes are valid for the prompt type
- `default` values are compatible with the prompt type
- `choices` are present when required
- `validate` attributes are only used where they make sense
- dependency expressions reference known prompts
- `.tera` template prompt references resolve to known prompts
- dependency method calls are valid for the referenced prompt type

## What Not to Prioritize Early

- Call hierarchy
- Type hierarchy
- Complex refactorings beyond rename
- Workspace-wide indexing before document-local features are solid
- Deep async infrastructure before the server can already parse and diagnose one
  document well
