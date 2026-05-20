//! Handler for the LSP `textDocument/formatting` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_formatting>
//!
//! Clients send this request when the user asks the editor to format the whole
//! document. The server responds with text edits that the client applies to the
//! current buffer. Returning an empty edit list means the document is already
//! formatted; returning `null` means the server has no formatting result for
//! the requested document.
//!
//! For Achitekfiles, this handler currently performs a simple full-document
//! layout pass: it trims each line, applies two-space indentation for nested
//! blocks, and returns a single full-document replacement edit when the text
//! changes.
use crate::server::ServerState;
use lsp_types::{DocumentFormattingParams, Position, Range, TextEdit};

/// Handles a `textDocument/formatting` request.
///
/// The request URI is used to find the current in-memory document. If the
/// document is known, the handler formats its text and returns either no edits
/// or one full-document replacement edit. If the document is unknown, the
/// handler returns `null`.
pub fn handle(
    state: &ServerState,
    params: DocumentFormattingParams,
) -> anyhow::Result<Option<Vec<TextEdit>>> {
    if let Some(document) = state.documents.get(params.text_document.uri.as_str()) {
        let formatted = format_achitek_source(&document.text);

        if formatted == document.text {
            Ok(Some(Vec::new()))
        } else {
            Ok(Some(vec![TextEdit {
                range: full_document_range(&document.text),
                new_text: formatted,
            }]))
        }
    } else {
        Ok(None)
    }
}

/// Formats Achitek source using simple brace-based indentation.
fn format_achitek_source(source: &str) -> String {
    let mut formatted = String::new();
    let mut indent = 0usize;

    for raw_line in source.lines() {
        let line = raw_line.trim();

        if line.starts_with('}') {
            indent = indent.saturating_sub(1);
        }

        if line.is_empty() {
            formatted.push('\n');
        } else {
            formatted.push_str(&"  ".repeat(indent));
            formatted.push_str(line);
            formatted.push('\n');
        }

        if line.ends_with('{') {
            indent += 1;
        }
    }

    formatted
}

/// Returns the LSP range covering the entire source document.
fn full_document_range(source: &str) -> Range {
    let last_line = source.lines().last().unwrap_or("");

    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: u32::try_from(source.lines().count()).expect("line count should fit into u32"),
            character: u32::try_from(last_line.len()).expect("line length should fit into u32"),
        },
    }
}
