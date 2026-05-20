use crate::utils;
use achitek_ls::{
    handlers::request::handle_workspace_symbol,
    server::{Document, Documents, ServerState},
};
use indoc::indoc;
use lsp_server::{Connection, Request, RequestId};
use lsp_types::{
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
    request::{Request as LspRequest, WorkspaceSymbolRequest},
};

#[test]
fn workspace_symbol_finds_prompt_symbols_matching_query() -> anyhow::Result<()> {
    let (server_connection, client_connection) = Connection::memory();
    let uri = "file:///workspace/Achitekfile";
    let request = Request::new(
        RequestId::from(1_i32),
        WorkspaceSymbolRequest::METHOD.to_owned(),
        WorkspaceSymbolParams {
            query: "project".to_owned(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let state = ServerState {
        documents: Documents::from([(
            uri.to_owned(),
            Document {
                version: 1,
                text: source(),
            },
        )]),
        ..Default::default()
    };
    let params = serde_json::from_value(request.params.clone())?;
    let result = handle_workspace_symbol(&state, params)?;

    utils::send_response(&server_connection, &request, result)?;

    let response = utils::request_response_sink(&client_connection)?;
    let result: Option<WorkspaceSymbolResponse> =
        serde_json::from_value(response.result.expect("response should contain a result"))?;
    let Some(WorkspaceSymbolResponse::Flat(symbols)) = result else {
        panic!("expected flat workspace symbols");
    };
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "project_name");

    Ok(())
}

fn source() -> String {
    indoc! {r#"
        blueprint {
          version = "1.0.0"
          name = "minimal"
        }

        prompt "project_name" {
          type = string
        }
    "#}
    .to_owned()
}
