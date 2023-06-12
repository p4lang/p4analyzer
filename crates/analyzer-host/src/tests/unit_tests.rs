use std::sync::Arc;

use analyzer_abstractions::lsp_types::{
	Position, TextDocumentIdentifier, TextDocumentPositionParams, Url, WorkDoneProgressParams,
};
use serde_json::Value;

use crate::{
	fs::LspEnumerableFileSystem,
	fsm::LspProtocolMachine,
	json_rpc::message::{Message, Notification, Request},
	lsp::{request::RequestManager, state::LspServerState},
};

#[test]
fn test_lsp_states() {
	let rm = RequestManager::new(async_channel::unbounded::<Message>());
	let mut lsp =
		LspProtocolMachine::new(None, rm.clone(), Arc::new(Box::new(LspEnumerableFileSystem::new(rm.clone()))));
	assert_eq!(lsp.current_state(), LspServerState::ActiveUninitialized);
	assert_eq!(lsp.is_active(), true);

	let mut params = serde_json::json!(analyzer_abstractions::lsp_types::InitializeParams { ..Default::default() });
	let mut message = Message::Request(Request { id: 0.into(), method: String::from("initialize"), params: params });
	let mut output = async_io::block_on(lsp.process_message(Arc::new(message)));
	assert!(output.is_ok());
	assert_eq!(lsp.current_state(), LspServerState::Initializing);
	assert_eq!(lsp.is_active(), true);

	let url = Url::parse("https://example.net").unwrap();
	let hover_params = analyzer_abstractions::lsp_types::HoverParams {
		text_document_position_params: TextDocumentPositionParams {
			text_document: TextDocumentIdentifier { uri: url },
			position: Position { line: 0, character: 0 },
		},
		work_done_progress_params: WorkDoneProgressParams { work_done_token: None },
	};
	params = serde_json::json!(hover_params);
	message = Message::Request(Request { id: 0.into(), method: String::from("textDocument/hover"), params: params });
	output = async_io::block_on(lsp.process_message(Arc::new(message)));
	assert!(output.is_ok());
	assert_eq!(lsp.current_state(), LspServerState::Initializing);
	assert_eq!(lsp.is_active(), true);

	params = serde_json::json!(analyzer_abstractions::lsp_types::InitializedParams {});
	message = Message::Notification(Notification { method: String::from("initialized"), params: params });
	output = async_io::block_on(lsp.process_message(Arc::new(message)));
	assert!(output.is_ok());
	assert_eq!(lsp.current_state(), LspServerState::ActiveInitialized);
	assert_eq!(lsp.is_active(), true);

	params = serde_json::json!(hover_params);
	message = Message::Notification(Notification { method: String::from("textDocument/hover"), params: params });
	output = async_io::block_on(lsp.process_message(Arc::new(message)));
	assert!(output.is_err());
	assert_eq!(lsp.current_state(), LspServerState::ActiveInitialized);
	assert_eq!(lsp.is_active(), true);

	message = Message::Request(Request { id: 0.into(), method: String::from("shutdown"), params: Value::Null });
	output = async_io::block_on(lsp.process_message(Arc::new(message)));
	assert!(output.is_ok());
	assert_eq!(lsp.current_state(), LspServerState::ShuttingDown);
	assert_eq!(lsp.is_active(), true);

	message = Message::Notification(Notification { method: String::from("exit"), params: Value::Null });
	output = async_io::block_on(lsp.process_message(Arc::new(message)));
	assert!(output.is_ok());
	assert_eq!(lsp.current_state(), LspServerState::Stopped);
	assert_eq!(lsp.is_active(), false);
}
