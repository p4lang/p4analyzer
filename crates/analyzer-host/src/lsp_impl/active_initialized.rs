use std::sync::Arc;
use async_rwlock::RwLock as AsyncRwLock;

use analyzer_abstractions::{
	lsp_types::{
		notification::{Exit, Notification, SetTrace},
		request::{Completion, HoverRequest, Shutdown},
		CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
		Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, SetTraceParams,
	},
	tracing::info,
};

use crate::lsp::{
	dispatch::Dispatch, dispatch_target::HandlerResult, state::LspServerState, DispatchBuilder,
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
	_: Arc<AsyncRwLock<State>>,
) -> HandlerResult<Option<CompletionResponse>> {
	let _ = params.text_document_position.position.line;

	let data = CompletionList {
		is_incomplete: false,
		items: vec![
			CompletionItem {
				label: String::from("p4"),
				kind: Some(CompletionItemKind::FILE),
				..Default::default()
			},
			CompletionItem {
				label: String::from("rules"),
				kind: Some(CompletionItemKind::FILE),
				..Default::default()
			},
		],
	};

	Ok(Some(CompletionResponse::List(data)))
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
