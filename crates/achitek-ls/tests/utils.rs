use lsp_server::{Connection, Message, Request, Response};
use lsp_types::PublishDiagnosticsParams;
use std::path::PathBuf;

pub const TEST_URI: &str = "file:///workspace/achitekfile";

#[allow(dead_code)]
pub fn achitekfile() -> String {
    indoc::indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }
        "#}
    .to_owned()
}

pub fn achitekfile_with_prompt() -> String {
    indoc::indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
              help = "Project name"
            }
        "#}
    .to_owned()
}

pub fn request_response_sink(connection: &Connection) -> anyhow::Result<Response> {
    match connection.receiver.recv()? {
        Message::Response(response) => Ok(response),
        message => anyhow::bail!("expected response, got {message:?}"),
    }
}

pub fn send_response(
    connection: &Connection,
    request: &Request,
    result: impl serde::Serialize,
) -> anyhow::Result<()> {
    connection.sender.send(Message::Response(Response::new_ok(
        request.id.clone(),
        result,
    )))?;
    Ok(())
}

#[allow(dead_code)]
pub fn published_diagnostics_sink(
    connection: &Connection,
) -> anyhow::Result<PublishDiagnosticsParams> {
    match connection.receiver.recv()? {
        Message::Notification(notification)
            if notification.method == "textDocument/publishDiagnostics" =>
        {
            Ok(serde_json::from_value(notification.params)?)
        }
        message => anyhow::bail!("expected publishDiagnostics, got {message:?}"),
    }
}

pub fn temp_dir(prefix: &str) -> anyhow::Result<PathBuf> {
    Ok(std::env::temp_dir().join(format!(
        "{prefix}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    )))
}
