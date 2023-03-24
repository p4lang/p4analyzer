use std::sync::Arc;
use analyzer_core::base_abstractions::FileId;
use async_rwlock::RwLock as AsyncRwLock;

use analyzer_abstractions::{
	lsp_types::{
		notification::{Exit, Notification, SetTrace, DidOpenTextDocument, DidChangeTextDocument, DidCloseTextDocument, DidSaveTextDocument},
		request::{Completion, HoverRequest, Shutdown},
		CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
		Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position, SetTraceParams, DidOpenTextDocumentParams, DidChangeTextDocumentParams,
		DidCloseTextDocumentParams, DidSaveTextDocumentParams,
	},
	tracing::info,
};

use crate::lsp::{
	dispatch::Dispatch, dispatch_target::{HandlerResult, HandlerError}, state::LspServerState, DispatchBuilder,
};

use super::state::State;

/// Builds and then returns a dispatcher handling the [`LspServerState::ActiveUninitialized`] state.
pub(crate) fn create_dispatcher() -> Box<dyn Dispatch<State> + Send + Sync + 'static> {
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
			.for_notification_with_options::<Exit, _>(on_exit, |mut options| {
				options.transition_to(LspServerState::Stopped)
			})
			.build(),
	)
}

async fn on_shutdown(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> {
	Ok(())
}

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
	let analyzer = state.write().await.analyzer.clone();
	let analyzer = analyzer.unwrap();

	let uri = params.text_document_position.text_document.uri.as_str();
	let file_id = analyzer.file_id(uri);

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

	let data = CompletionList {
		is_incomplete: false,
		items,
	};

	let shown_tokens = lexed.iter().map(|(file_id, tk, span)|
		format!("{tk:?}: {file_id:?} {span:?}")
	).collect::<Vec<_>>();
	info!("input: {:?}\n{:?}", input, shown_tokens);
	info!("files: {:?}", analyzer.files());

	Ok(Some(CompletionResponse::List(data)))
}

async fn on_text_document_did_open(
	_: LspServerState,
	params: DidOpenTextDocumentParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let analyzer = state.write().await.analyzer.clone();
	let mut analyzer = analyzer.unwrap();

	let file_id = analyzer.file_id(params.text_document.uri.as_str());
	analyzer.update(file_id, params.text_document.text);

	Ok(())
}

async fn on_text_document_did_change(
	_: LspServerState,
	params: DidChangeTextDocumentParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let state = state.write().await;
	let mut analyzer = state.analyzer.unwrap();

	let uri = params.text_document.uri.as_str();
	let file_id = analyzer.file_id(uri);
	// FIXME: potentially unnecessary allocation
	let mut input = match analyzer.input(file_id) {
		Some(i) => i.to_string(),
		None => return Err(HandlerError::new_with_data(
			"received a didChange notification for an unknown file",
			Some(uri)
		))
	};

	for change in params.content_changes {
		let analyzer_abstractions::lsp_types::TextDocumentContentChangeEvent {
			range,
			range_length: _,
			text,
		} = change;
		if let Some(range) = range {
			let range = lsp_range_to_byte_range(&input, range);
			info!("replacing range {:?} of {:?} with {:?}", range, &input[range.clone()], text);
			input.replace_range(range, &text);
		} else {
			input = text;
		}
	}

	// TODO: avoid cloning
	analyzer.update(file_id, input.clone());
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
	state: Arc<AsyncRwLock<State>>
) -> HandlerResult<()> {
	let state = state.write().await;
	let mut analyzer = state.analyzer.unwrap();

	analyzer.delete(params.text_document.uri.as_str());

	Ok(())
}

async fn on_text_document_did_save(
	_: LspServerState,
	params: DidSaveTextDocumentParams,
	state: Arc<AsyncRwLock<State>>
) -> HandlerResult<()> {
	let state = state.write().await;
	let mut analyzer = state.analyzer.unwrap();

	if let Some(text) = params.text {
		info!("Syncing buffer on save.");
		let file_id = analyzer.file_id(params.text_document.uri.as_str());
		let diagnostics = process_diagnostics(&analyzer, file_id, &text);
		// TODO: report diagnostics, and process *after* the update below!
		analyzer.update(file_id, text);
	}

	Ok(())
}

async fn on_set_trace(
	_: LspServerState,
	params: SetTraceParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let read_state = state.read().await;

	// If a trace value accessor is available, then set it using the trace value received in the
	// parameters.
	if let Some(tv) = &read_state.trace_value {
		info!(
			method = SetTrace::METHOD,
			"Setting trace level to {:?}", params.value
		);

		tv.set(params.value);
	}

	Ok(())
}

/// Responds to an 'exit' notification from the LSP client.
async fn on_exit(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> {
	Ok(())
}

fn process_diagnostics(analyzer: &analyzer_core::Analyzer, file_id: FileId, input: &str) -> Vec<analyzer_abstractions::lsp_types::Diagnostic> {
	let diagnostics = analyzer.diagnostics(file_id);

	diagnostics.into_iter().map(|d| {
		use analyzer_abstractions::lsp_types::{Diagnostic, DiagnosticSeverity};
		use analyzer_core::base_abstractions::Severity;

		Diagnostic {
			range: byte_range_to_lsp_range(input, d.location),
			severity: Some(match d.severity {
				Severity::Info => DiagnosticSeverity::INFORMATION,
				Severity::Hint => DiagnosticSeverity::HINT,
				Severity::Warning => DiagnosticSeverity::WARNING,
				Severity::Error => DiagnosticSeverity::ERROR,
			}),
			message: d.message,
			..Default::default()
		}
	}).collect()
}

fn lsp_range_to_byte_range(
	input: &str,
	range: analyzer_abstractions::lsp_types::Range,
) -> std::ops::Range<usize> {
	let start = position_to_byte_offset(input, range.start);
	let end = position_to_byte_offset(input, range.end);
	start..end
}

fn byte_range_to_lsp_range(input: &str, range: std::ops::Range<usize>) -> analyzer_abstractions::lsp_types::Range {
	let start = byte_offset_to_position(input, range.start);
	let end = byte_offset_to_position(input, range.end);
	analyzer_abstractions::lsp_types::Range::new(start, end)
}

// FIXME: UTF8?
fn position_to_byte_offset(input: &str, pos: Position) -> usize {
	let Position {
		line: line_index,
		character,
	} = pos;
	let line_index = line_index as usize;

	let mut offset = 0;
	for (index, line) in input.split_inclusive('\n').enumerate() {
		if index == line_index {
			offset += line.as_bytes().len().min(character as usize);
			break;
		}
		offset += line.as_bytes().len()
	}
	offset
}

fn byte_offset_to_position(input: &str, offset: usize) -> Position {
	let mut line_number = 0;
	let mut offset_counter = 0;

	for (index, line) in input.split_inclusive('\n').enumerate() {
		line_number = index;
		if offset_counter + line.len() > offset {
			break
		}
		offset_counter += line.len();
	}

	Position::new(line_number as u32, (offset - offset_counter) as u32)
}
