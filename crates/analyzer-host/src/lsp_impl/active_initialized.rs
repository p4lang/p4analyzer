use analyzer_core::{base_abstractions::FileId, lsp_file::ChangeEvent};
use async_rwlock::RwLock as AsyncRwLock;
use std::sync::Arc;

use analyzer_abstractions::{
	lsp_types::{
		notification::{
			DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
			DidSaveTextDocument, Exit, SetTrace,
		},
		request::{Completion, HoverRequest, Shutdown},
		CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
		DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
		DidOpenTextDocumentParams, DidSaveTextDocumentParams, FileChangeType, Hover, HoverContents, HoverParams,
		MarkupContent, MarkupKind, Position, Range, SetTraceParams, Url,
	},
	tracing::{error, info},
};

use crate::{
	fsm::LspServerStateDispatcher,
	lsp::{
		dispatch_target::{HandlerError, HandlerResult},
		state::LspServerState,
		DispatchBuilder,
	},
};

use super::state::State;

/// Builds and then returns a dispatcher handling the [`LspServerState::ActiveUninitialized`] state.
pub(crate) fn create_dispatcher() -> LspServerStateDispatcher {
	Box::new(
		DispatchBuilder::<State>::new(LspServerState::ActiveInitialized)
			.for_request_with_options::<Shutdown, _>(on_shutdown, |mut options| {
				options.transition_to(LspServerState::ShuttingDown)
			})
			.for_request::<HoverRequest, _>(on_text_document_hover)
			.for_request::<Completion, _>(on_text_document_completion)
			.for_notification::<DidChangeTextDocument, _>(on_text_document_did_change)
			.for_notification::<DidCloseTextDocument, _>(on_text_document_did_close)
			.for_notification::<DidOpenTextDocument, _>(on_text_document_did_open)
			.for_notification::<DidSaveTextDocument, _>(on_text_document_did_save)
			.for_notification::<SetTrace, _>(on_set_trace)
			.for_notification::<DidChangeWatchedFiles, _>(on_watched_file_change)
			.for_notification_with_options::<Exit, _>(on_exit, |mut options| {
				options.transition_to(LspServerState::Stopped)
			})
			.build(),
	)
}

async fn on_shutdown(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> { Ok(()) }

async fn on_text_document_hover(
	_: LspServerState,
	params: HoverParams,
	_: Arc<AsyncRwLock<State>>,
) -> HandlerResult<Option<Hover>> {
	let line = params.text_document_position_params.position.line;
	let character = params.text_document_position_params.position.character;

	let hover = Hover {
		range: None,
		contents: HoverContents::Markup(MarkupContent {
			kind: MarkupKind::Markdown,
			value: format!("Hovering over Ln *{}*, Col *{}*.", line, character),
		}),
	};

	Ok(Some(hover))
}

async fn on_text_document_completion(
	_: LspServerState,
	params: CompletionParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<Option<CompletionResponse>> {
	use itertools::Itertools;

	let state = state.read().await;
	let uri = params.text_document_position.text_document.uri;
	let file = state.workspaces().get_file(uri.clone());

	match file.get_parsed_unit().await {
		Ok(file_id) => {
			let analyzer = state.analyzer.unwrap();

			let (input, lexed) = match (analyzer.input(file_id), analyzer.preprocessed(file_id)) {
				(Some(i), Some(l)) => (i, l),
				_ => return Ok(Some(CompletionResponse::Array(vec![]))),
			};

			let items = lexed
				.iter()
				.flat_map(|(_, token, _)| match token {
					analyzer_core::lexer::Token::Identifier(name) => Some(name),
					_ => None,
				})
				.unique()
				.map(|label| CompletionItem {
					label: label.to_string(),
					kind: Some(CompletionItemKind::FILE),
					..Default::default()
				})
				.collect();

			let data = CompletionList { is_incomplete: false, items };

			let shown_tokens =
				lexed.iter().map(|(file_id, tk, span)| format!("{tk:?}: {file_id:?} {span:?}")).collect::<Vec<_>>();
			info!("input: {:?}\n{:?}", input, shown_tokens);
			info!("files: {:?}", analyzer.files());

			Ok(Some(CompletionResponse::List(data)))
		}
		Err(err) => {
			error!(file_uri = uri.as_str(), "Could not query completions. Index error: {}", err);

			Err(HandlerError::new("Could not query completions for document."))
		}
	}
}

async fn on_text_document_did_open(
	_: LspServerState,
	params: DidOpenTextDocumentParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let state = state.write().await;
	let file = state.workspaces().get_file(params.text_document.uri.clone());
	let mut analyzer = state.analyzer.unwrap();

	let file_id = analyzer.file_id(params.text_document.uri.as_str());
	analyzer.update(file_id, &params.text_document.text);

	file.open_or_update(file_id);

	Ok(())
}

async fn on_text_document_did_change(
	_: LspServerState,
	params: DidChangeTextDocumentParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let state = state.write().await;
	let file = state.workspaces().get_file(params.text_document.uri.clone());
	let mut analyzer = state.analyzer.unwrap();

	let uri = params.text_document.uri.as_str();
	let file_id = analyzer.file_id(uri);
	// FIXME: potentially unnecessary allocation
	let mut input = match analyzer.input(file_id) {
		Some(i) => i.to_string(),
		None => {
			return Err(HandlerError::new_with_data("received a didChange notification for an unknown file", Some(uri)))
		}
	};

	use analyzer_abstractions::lsp_types::TextDocumentContentChangeEvent;
	let event_change = params
		.content_changes
		.into_iter()
		.map(|TextDocumentContentChangeEvent { range, text, range_length: _ }| {
			use analyzer_core::lsp_file as core;
			let range = range.map(|Range { start, end }| core::Range {
				start: core::Position { line: start.line as usize, character: start.character as usize },
				end: core::Position { line: end.line as usize, character: end.character as usize },
			});
			ChangeEvent { range, text }
		})
		.collect();

	analyzer.file_change_event(file_id, &event_change);

	file.open_or_update(file_id);
	let diagnostics = process_diagnostics(&analyzer, file_id, &input);

	// TODO: report diagnostics
	// Ok(Some(PublishDiagnosticsParams {
	// 	uri: params.text_document.uri,
	// 	diagnostics,
	// 	version: None,
	// }))
	Ok(())
}

async fn on_text_document_did_close(
	_: LspServerState,
	params: DidCloseTextDocumentParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let state = state.write().await;
	let file = state.workspaces().get_file(params.text_document.uri.clone());
	let mut analyzer = state.analyzer.unwrap();

	analyzer.delete(params.text_document.uri.as_str());
	file.close();

	Ok(())
}

async fn on_text_document_did_save(
	_: LspServerState,
	params: DidSaveTextDocumentParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	if let Some(text) = params.text {
		let state = state.write().await;
		let file = state.workspaces().get_file(params.text_document.uri.clone());
		let mut analyzer = state.analyzer.unwrap();

		info!("Syncing buffer on save.");
		let file_id = analyzer.file_id(params.text_document.uri.as_str());
		let diagnostics = process_diagnostics(&analyzer, file_id, &text);
		// TODO: report diagnostics, and process *after* the update below!
		analyzer.update(file_id, &text);
		file.open_or_update(file_id);
	}

	Ok(())
}

async fn on_set_trace(_: LspServerState, params: SetTraceParams, state: Arc<AsyncRwLock<State>>) -> HandlerResult<()> {
	let state = state.read().await;

	state.set_trace_value(params.value);

	Ok(())
}

async fn created_file(uri: &Url, state: &Arc<AsyncRwLock<State>>) {
	// workspaces should be created in the initilize state
	let file = state.write().await.workspaces().get_file(uri.clone());

	// check if file is in client memory
	if file.is_open_in_ide() {
		return; // we don't need to query filesystem
	}

	match file.get_parsed_unit().await {
		Ok(file_id) => {
			let lock = state.write().await;
			let content = lock.file_system.file_contents(uri.clone()).await.unwrap_or_default();
			lock.analyzer.unwrap().update(file_id, &content);
			info!("{} file updated from file system", uri.path());
		}
		Err(err) => {
			error!(uri = uri.as_str(), "Could not query completions. Index error: {}", err);
		}
	}
}

async fn deleted_file(uri: &Url, state: &Arc<AsyncRwLock<State>>) {
	// workspaces should be created in the initilize state
	let file = state.write().await.workspaces().get_file(uri.clone());

	// check if file is in client memory
	if file.is_open_in_ide() {
		return; // we don't need to query filesystem
	}

	state.write().await.analyzer.unwrap().delete(uri.as_str());
	info!("{} file deleted from file system", uri.path());
}

async fn on_watched_file_change(
	_: LspServerState,
	params: DidChangeWatchedFilesParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	for event in &params.changes {
		match event.typ {
			FileChangeType::CREATED => created_file(&event.uri, &state).await,
			FileChangeType::CHANGED => created_file(&event.uri, &state).await, // Does the same
			FileChangeType::DELETED => deleted_file(&event.uri, &state).await,
			_ => panic!("Type not supported in 1.17 specification"),
		}
	}

	Ok(())
}

/// Responds to an 'exit' notification from the LSP client.
async fn on_exit(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> { Ok(()) }

fn process_diagnostics(
	analyzer: &analyzer_core::Analyzer,
	file_id: FileId,
	input: &str,
) -> Vec<analyzer_abstractions::lsp_types::Diagnostic> {
	let diagnostics = analyzer.diagnostics(file_id);
	let lsp = analyzer.get_file(file_id);
	diagnostics
		.into_iter()
		.map(|d| {
			use analyzer_abstractions::lsp_types::{Diagnostic, DiagnosticSeverity};
			use analyzer_core::base_abstractions::Severity;

			let std::ops::Range { start, end } = d.location;
			let (start, end) = (lsp.byte_to_lsp(start), lsp.byte_to_lsp(end));
			let range = Range {
				start: Position { line: start.line as u32, character: start.character as u32 },
				end: Position { line: end.line as u32, character: end.character as u32 },
			};
			Diagnostic {
				range,
				severity: Some(match d.severity {
					Severity::Info => DiagnosticSeverity::INFORMATION,
					Severity::Hint => DiagnosticSeverity::HINT,
					Severity::Warning => DiagnosticSeverity::WARNING,
					Severity::Error => DiagnosticSeverity::ERROR,
				}),
				message: d.message,
				..Default::default()
			}
		})
		.collect()
}
