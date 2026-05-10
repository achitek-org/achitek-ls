use lsp_server::{Connection, Message};
use lsp_types::PublishDiagnosticsParams;

pub const TEST_URI: &str = "file:///workspace/achitekfile";

pub fn achitekfile() -> String {
    indoc::indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }
        "#}
    .to_owned()
}

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
