//! Handler for the LSP `textDocument/hover` request.
//!
//! Spec: <https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_hover>
//!
//! Clients send this request when the user hovers over a position in the
//! document. Editors use the response to show contextual documentation near the
//! cursor.

use crate::{editor, server::ServerState};
use anyhow::Context;
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position, Range};

/// Handles a `textDocument/hover` request.
pub fn handle(state: &ServerState, params: HoverParams) -> anyhow::Result<Option<Hover>> {
    let text_document_position = params.text_document_position_params;

    if let Some(document) = state
        .documents
        .get(text_document_position.text_document.uri.as_str())
    {
        let editor_buffer = editor::from_source(&document.text).with_context(|| {
            format!(
                "failed to analyze document `{:?}`",
                text_document_position.text_document.uri
            )
        })?;
        Ok(editor_buffer
            .hover(to_text_position(text_document_position.position))
            .map(to_lsp_hover))
    } else {
        Ok(None)
    }
}

/// Converts editor hover content into an LSP hover response.
fn to_lsp_hover(hover: editor::Hover) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover.contents().to_owned(),
        }),
        range: Some(to_lsp_range(hover.range())),
    }
}

/// Converts an LSP position into an editor position.
fn to_text_position(position: Position) -> achitekfile::TextPosition {
    achitekfile::TextPosition {
        line: usize::try_from(position.line).expect("line should fit into usize"),
        byte: usize::try_from(position.character).expect("character should fit into usize"),
    }
}

/// Converts an editor text range into an LSP range.
fn to_lsp_range(range: achitekfile::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start),
        end: to_lsp_position(range.end),
    }
}

/// Converts an editor text position into an LSP position.
fn to_lsp_position(position: achitekfile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}
