use lsp_types::{
    CompletionOptions, FoldingRangeProviderCapability, HoverProviderCapability, OneOf,
    RenameOptions, SelectionRangeProviderCapability, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
};

/// Returns the LSP capabilities advertised by the language server.
///
/// Each enabled capability tells the client that the server can handle the
/// corresponding request or notification. Keep this list aligned with the
/// handlers implemented by the server; clients may call any capability that is
/// advertised here.
pub fn make() -> ServerCapabilities {
    ServerCapabilities {
        // Provides completion items for keywords, attributes, prompt types, and
        // dependency-expression helpers.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_completion
        completion_provider: Some(CompletionOptions::default()),
        // Lets clients jump from a prompt reference to the prompt definition.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_definition
        definition_provider: Some(OneOf::Left(true)),
        // Lets clients request a full-document formatting edit.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_formatting
        document_formatting_provider: Some(OneOf::Left(true)),
        // Lets clients show an outline of blueprint, prompt, and validate blocks.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_documentSymbol
        document_symbol_provider: Some(OneOf::Left(true)),
        // Lets clients fold block ranges such as blueprint, prompt, and validate.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_foldingRange
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        // Lets clients request explanatory text for the symbol under the cursor.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_hover
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        // Lets clients find all usages of a prompt or other referenceable symbol.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_references
        references_provider: Some(OneOf::Left(true)),
        // Lets clients rename a symbol after validating that the cursor is on a
        // renameable target.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_rename
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        // Lets clients expand selections by syntax tree ranges.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_selectionRange
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        // Tells clients to send open, close, and full-content change
        // notifications so diagnostics and features use the editor buffer.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_synchronization
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                ..TextDocumentSyncOptions::default()
            },
        )),
        // Lets clients search symbols across the workspace.
        // Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#workspace_symbol
        workspace_symbol_provider: Some(OneOf::Left(true)),
        ..ServerCapabilities::default()
    }
}
