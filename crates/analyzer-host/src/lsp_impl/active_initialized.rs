use analyzer_core::base_abstractions::FileId;
use async_rwlock::RwLock as AsyncRwLock;
use std::sync::Arc;

use analyzer_abstractions::{
	lsp_types::{
		notification::{
			DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
			DidSaveTextDocument, Exit, PublishDiagnostics, SetTrace,
		},
		request::{Completion, GotoDefinition, HoverRequest, Shutdown},
		CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
		DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
		DidOpenTextDocumentParams, DidSaveTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, Hover,
		HoverContents, HoverParams, Location, MarkupContent, MarkupKind, Position, SetTraceParams,
	},
	tracing::{error, info},
};

use crate::{
	fsm::LspServerStateDispatcher,
	lsp::{
		dispatch::Dispatch,
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
			.for_request::<GotoDefinition, _>(on_goto_definition)
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

async fn on_goto_definition(
	_: LspServerState,
	params: GotoDefinitionParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<Option<GotoDefinitionResponse>> {
	// TODO: move this (or maybe just references) to analyzer-core
	use analyzer_core::parser::{ast::*, *};

	let state = state.read().await;
	let uri = params.text_document_position_params.text_document.uri;
	let file = state.workspaces().get_file(uri.clone());

	let file_id = if let Ok(file_id) = file.get_parsed_unit().await {
		Ok(file_id)
	} else {
		Err(HandlerError::new("File not found"))
	}?;

	let analyzer = state.analyzer.unwrap();

	let input = analyzer.input(file_id).ok_or(HandlerError::new("db doesn't have the input string"))?;
	let cursor = position_to_byte_offset(input, params.text_document_position_params.position);
	let tree = analyzer.parsed(file_id).ok_or(HandlerError::new("not parsed"))?;
	let cumulative_sum = analyzer.cumulative_sum(file_id).ok_or(HandlerError::new("token offsets not found"))?;

	let root = SyntaxNode::new_root(p4_grammar::get_grammar().into(), tree);
	// find the identifier under cursor
	let ident = preorder(0, root.clone())
		.map(|(_, n)| n)
		.filter_map(Ident::cast)
		.find(|ident| ident.text_span(cumulative_sum).contains(&cursor));

	Ok(ident.and_then(|ident| {
		// find the matching definition
		let definition = preorder(0, root)
			.map(|(_, n)| n)
			.filter_map(Definition::cast)
			.flat_map(|d| d.ident())
			.find(|param| param.as_str() == ident.as_str());

		definition.map(|def| {
			let span = def.text_span(cumulative_sum);
			let range = byte_range_to_lsp_range(input, span);
			GotoDefinitionResponse::Scalar(Location::new(uri, range))
		})
	}))
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

			let parsed = analyzer.parsed(file_id);
			info!("parsed file {parsed:?}");

			let items = parsed
				.into_iter()
				.flat_map(|tree| {
					use analyzer_core::parser::{ast::*, *};

					let root = SyntaxNode::new_root(p4_grammar::get_grammar().into(), tree);
					preorder(0, root)
						.map(|(_, node)| node)
						.filter_map(Definition::cast)
						.flat_map(|def| def.ident())
						.map(|ident| CompletionItem {
							label: ident.as_str().to_string(),
							kind: Some(CompletionItemKind::VARIABLE),
							..Default::default()
						})
				})
				.chain(
					lexed
						.iter()
						.flat_map(|(_, token, _)| match token {
							analyzer_core::lexer::Token::Identifier(name) => Some(name),
							_ => None,
						})
						.map(|label| CompletionItem {
							label: label.to_string(),
							kind: Some(CompletionItemKind::FILE),
							..Default::default()
						}),
				)
				.unique_by(|item| item.label.clone())
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

	analyzer.update(file_id, params.text_document.text);

	file.open_or_update(file_id);

	Ok(())
}

async fn on_text_document_did_change(
	_: LspServerState,
	params: DidChangeTextDocumentParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let diagnostics = {
		let state = state.write().await;
		let file = state.workspaces().get_file(params.text_document.uri.clone());
		let mut analyzer = state.analyzer.unwrap();

		let uri = params.text_document.uri.as_str();
		let file_id = analyzer.file_id(uri);
		// FIXME: potentially unnecessary allocation
		let mut input = match analyzer.input(file_id) {
			Some(i) => i.to_string(),
			None => {
				return Err(HandlerError::new_with_data(
					"received a didChange notification for an unknown file",
					Some(uri),
				))
			}
		};

		for change in params.content_changes {
			let analyzer_abstractions::lsp_types::TextDocumentContentChangeEvent { range, range_length: _, text } =
				change;
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
		file.open_or_update(file_id);
		process_diagnostics(&analyzer, file_id, &input)
	};

	state
		.read()
		.await
		.request_manager
		.send_notification::<PublishDiagnostics>(analyzer_abstractions::lsp_types::PublishDiagnosticsParams {
			uri: params.text_document.uri,
			diagnostics,
			version: None,
		})
		.await
		.map_err(|err| HandlerError::new_with_data("Could not send diagnostics", Some(err.to_string())))?;

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
		analyzer.update(file_id, text);
		file.open_or_update(file_id);
	}

	Ok(())
}

async fn on_set_trace(_: LspServerState, params: SetTraceParams, state: Arc<AsyncRwLock<State>>) -> HandlerResult<()> {
	let state = state.read().await;

	state.set_trace_value(params.value);

	Ok(())
}

async fn on_watched_file_change(
	_: LspServerState,
	params: DidChangeWatchedFilesParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let file_changes: Vec<String> = params
		.changes
		.into_iter()
		.map(|file_event| format!("({:?} {})", file_event.typ, file_event.uri))
		.collect();

	info!(file_changes = file_changes.join(", "), "Watched file changes.");

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

	diagnostics
		.into_iter()
		.map(|d| {
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
		})
		.collect()
}

fn lsp_range_to_byte_range(input: &str, range: analyzer_abstractions::lsp_types::Range) -> std::ops::Range<usize> {
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
	let Position { line: line_index, character } = pos;
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
			break;
		}
		offset_counter += line.len();
	}

	Position::new(line_number as u32, (offset - offset_counter) as u32)
}
