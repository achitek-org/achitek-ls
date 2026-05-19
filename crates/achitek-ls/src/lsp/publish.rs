//! LSP diagnostic publishing.
//!
//! This module is intentionally an adapter: Achitekfile and Tera diagnostics
//! are produced by their language crates, then converted into LSP diagnostics
//! here.

use crate::{lsp::project_diagnostics, server::ServerState, workspace::DocumentKind};
use anyhow::Context;
use lsp_server::{Connection, Message, Notification};
use lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, NumberOrString, Position,
    PublishDiagnosticsParams, Range, Uri,
};

pub fn publish(connection: &Connection, uri: &Uri, state: &ServerState) -> anyhow::Result<()> {
    let Some(document) = state.documents.get(uri.as_str()) else {
        return Ok(());
    };

    let mut diagnostics = diagnostics_for_document(state.document_kind(uri), uri, &document.text)?;
    if state.document_kind(uri) == DocumentKind::Achitekfile {
        diagnostics.extend(project_diagnostics::achitekfile_diagnostics(uri, state)?);
    }
    tracing::debug!(
        ?uri,
        version = document.version,
        count = diagnostics.len(),
        "publishing document diagnostics"
    );

    send_publish_diagnostics(
        connection,
        PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: Some(document.version),
        },
    )
}

pub fn publish_templates(
    connection: &Connection,
    uri: &Uri,
    state: &ServerState,
) -> anyhow::Result<()> {
    for (template_uri, diagnostics) in project_diagnostics::template_diagnostics(uri, state)? {
        tracing::debug!(
            ?uri,
            ?template_uri,
            count = diagnostics.len(),
            "publishing template diagnostics"
        );
        send_publish_diagnostics(
            connection,
            PublishDiagnosticsParams {
                uri: template_uri,
                diagnostics,
                version: None,
            },
        )?;
    }

    Ok(())
}

pub fn clear(connection: &Connection, uri: &Uri) -> anyhow::Result<()> {
    tracing::debug!(?uri, "clearing diagnostics");
    send_publish_diagnostics(
        connection,
        PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics: Vec::new(),
            version: None,
        },
    )
}

fn diagnostics_for_document(
    kind: DocumentKind,
    uri: &Uri,
    source: &str,
) -> anyhow::Result<Vec<LspDiagnostic>> {
    match kind {
        DocumentKind::TeraTemplate => {
            let analysis = terafile::analyze(source)
                .with_context(|| format!("failed to analyze `{uri:?}`"))?;
            Ok(analysis
                .diagnostics()
                .iter()
                .map(to_tera_lsp_diagnostic)
                .collect())
        }
        DocumentKind::Achitekfile => {
            let analysis = achitekfile::analyze(source)
                .with_context(|| format!("failed to analyze `{uri:?}`"))?;
            Ok(analysis
                .diagnostics()
                .iter()
                .map(to_achitek_lsp_diagnostic)
                .collect())
        }
        DocumentKind::Manifest | DocumentKind::Unknown => Ok(Vec::new()),
    }
}

fn send_publish_diagnostics(
    connection: &Connection,
    params: PublishDiagnosticsParams,
) -> anyhow::Result<()> {
    let notification = Notification::new("textDocument/publishDiagnostics".to_owned(), params);
    connection
        .sender
        .send(Message::Notification(notification))
        .context("failed to send publishDiagnostics notification")?;

    Ok(())
}

fn to_achitek_lsp_diagnostic(diagnostic: &achitekfile::Diagnostic) -> LspDiagnostic {
    LspDiagnostic {
        range: to_achitek_lsp_range(diagnostic.range()),
        severity: Some(to_achitek_lsp_severity(diagnostic.severity())),
        code: Some(NumberOrString::String(
            diagnostic.code().as_str().to_owned(),
        )),
        message: diagnostic.message().to_owned(),
        ..LspDiagnostic::default()
    }
}

fn to_tera_lsp_diagnostic(diagnostic: &terafile::Diagnostic) -> LspDiagnostic {
    LspDiagnostic {
        range: to_tera_lsp_range(diagnostic.range()),
        severity: Some(to_tera_lsp_severity(diagnostic.severity())),
        code: Some(NumberOrString::String(
            diagnostic.code().as_str().to_owned(),
        )),
        message: diagnostic.message().to_owned(),
        ..LspDiagnostic::default()
    }
}

fn to_achitek_lsp_severity(severity: achitekfile::Severity) -> DiagnosticSeverity {
    match severity {
        achitekfile::Severity::Error => DiagnosticSeverity::ERROR,
        achitekfile::Severity::Warning => DiagnosticSeverity::WARNING,
        achitekfile::Severity::Hint => DiagnosticSeverity::HINT,
    }
}

fn to_tera_lsp_severity(severity: terafile::Severity) -> DiagnosticSeverity {
    match severity {
        terafile::Severity::Error => DiagnosticSeverity::ERROR,
        terafile::Severity::Warning => DiagnosticSeverity::WARNING,
        terafile::Severity::Hint => DiagnosticSeverity::HINT,
    }
}

fn to_achitek_lsp_range(range: achitekfile::TextRange) -> Range {
    Range {
        start: to_achitek_lsp_position(range.start),
        end: to_achitek_lsp_position(range.end),
    }
}

fn to_achitek_lsp_position(position: achitekfile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}

fn to_tera_lsp_range(range: terafile::TextRange) -> Range {
    Range {
        start: to_tera_lsp_position(range.start),
        end: to_tera_lsp_position(range.end),
    }
}

fn to_tera_lsp_position(position: terafile::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.line).expect("line should fit into u32"),
        character: u32::try_from(position.byte).expect("column should fit into u32"),
    }
}
