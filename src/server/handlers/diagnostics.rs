use crate::{
    analysis,
    server::{Documents, utils},
    syntax,
};
use anyhow::Context;
use lsp_server::{Connection, Message, Notification};
use lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location,
    Position, PublishDiagnosticsParams, Range, Uri,
};

pub fn publish(connection: &Connection, uri: &Uri, documents: &Documents) -> anyhow::Result<()> {
    if utils::is_template_uri(uri) {
        return publish_template(connection, uri, documents);
    }

    let Some(document) = documents.get(uri.as_str()) else {
        return Ok(());
    };

    let analysis = analysis::analyze(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
    let diagnostics = analysis
        .diagnostics()
        .iter()
        .map(|diagnostic| to_lsp_diagnostic(uri, diagnostic))
        .collect::<Vec<_>>();
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

pub fn publish_template(
    connection: &Connection,
    uri: &Uri,
    documents: &Documents,
) -> anyhow::Result<()> {
    let diagnostics = utils::template_diagnostics(uri, documents)?;
    tracing::debug!(
        ?uri,
        count = diagnostics.len(),
        "publishing current template diagnostics"
    );

    send_publish_diagnostics(
        connection,
        PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: documents.get(uri.as_str()).map(|document| document.version),
        },
    )
}

pub fn publish_templates(
    connection: &Connection,
    uri: &Uri,
    documents: &Documents,
) -> anyhow::Result<()> {
    for (template_uri, diagnostics) in utils::diagnostics(uri, documents)? {
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

fn to_lsp_diagnostic(uri: &Uri, diagnostic: &analysis::Diagnostic) -> LspDiagnostic {
    LspDiagnostic {
        range: to_lsp_range(diagnostic.range()),
        severity: Some(match diagnostic.severity() {
            analysis::Severity::Error => DiagnosticSeverity::ERROR,
            analysis::Severity::Warning => DiagnosticSeverity::WARNING,
            analysis::Severity::Information => DiagnosticSeverity::INFORMATION,
            analysis::Severity::Hint => DiagnosticSeverity::HINT,
        }),
        message: diagnostic.message().to_owned(),
        related_information: Some(
            diagnostic
                .related_information()
                .iter()
                .map(|info| DiagnosticRelatedInformation {
                    location: Location::new(uri.clone(), to_lsp_range(info.range())),
                    message: info.message().to_owned(),
                })
                .collect(),
        ),
        ..LspDiagnostic::default()
    }
}

fn to_lsp_range(range: syntax::TextRange) -> Range {
    Range {
        start: to_lsp_position(range.start_position),
        end: to_lsp_position(range.end_position),
    }
}

fn to_lsp_position(position: syntax::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.row).expect("line should fit into u32"),
        character: u32::try_from(position.column).expect("column should fit into u32"),
    }
}
