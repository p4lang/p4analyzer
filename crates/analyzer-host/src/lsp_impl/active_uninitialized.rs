use async_rwlock::RwLock as AsyncRwLock;
use std::sync::Arc;

use analyzer_abstractions::lsp_types::{
	notification::Exit, request::Initialize, CompletionOptions, DeclarationCapability, HoverProviderCapability,
	ImplementationProviderCapability, InitializeParams, InitializeResult, OneOf, SaveOptions, ServerCapabilities,
	ServerInfo, SignatureHelpOptions, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
	TextDocumentSyncSaveOptions, TypeDefinitionProviderCapability, WindowClientCapabilities, WorkDoneProgressOptions,
	WorkspaceFolder, WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities
};

use crate::{
	fsm::LspServerStateDispatcher,
	json_rpc::ErrorCode,
	lsp::{
		dispatch::Dispatch,
		dispatch_target::{HandlerError, HandlerResult},
		progress::ProgressManager,
		state::LspServerState,
		workspace::WorkspaceManager,
		DispatchBuilder
	}
};

use super::state::State;

/// Builds and then returns a dispatcher handling the [`LspServerState::ActiveUninitialized`] state.

pub(crate) fn create_dispatcher() -> LspServerStateDispatcher {

	Box::new(
		DispatchBuilder::<State>::new(LspServerState::ActiveUninitialized)
			.for_request_with_options::<Initialize, _>(on_initialize, |mut options| {

				options.transition_to(LspServerState::Initializing)
			})
			.for_unhandled_requests((ErrorCode::ServerNotInitialized, "An 'initialize' request is required."))
			.for_notification_with_options::<Exit, _>(on_exit, |mut options| {

				options.transition_to(LspServerState::Stopped)
			})
			.build()
	)
}

/// Responds to an 'initialize' request from the LSP client by returning a data structure that describes
/// the capabilities of the P4 Analyzer.
///
/// If the client capabilities do not include support for workspace watched files, then an error is raised
/// since the P4 Analyzer currently relies on the LSP client to watch for file changes in the workspaces.

async fn on_initialize(
	_: LspServerState,
	params: InitializeParams,
	state: Arc<AsyncRwLock<State>>
) -> HandlerResult<InitializeResult> {

	initialize_client_dependant_state(state.clone(), params.workspace_folders, params.capabilities.window).await;

	let state = state.read().await;

	// If the server is being initialized with a trace value, then set use to set the current LSP tracing layer.
	if let Some(trace_value) = params.trace {

		state.set_trace_value(trace_value);
	}

	// If the server has been started without any workspace context, then simply return our 'default' capability.
	if !state.has_workspaces() {

		return Ok(create_initialize_result(false));
	}

	// With a workspace context in place, the P4 Analyzer depends on the LSP client to notify it of external file changes.
	// If the client supports that capability, then start indexing
	match params.capabilities.workspace {
		Some(capabilities)
			if capabilities.did_change_watched_files != None => Ok(create_initialize_result(true)),
		_ => {
			Err(
				HandlerError::new(
					"P4 Analyzer requires the Watched Files ('workspace.didChangeWatchedFiles') capability from the LSP client."))
		}
	}
}

/// Responds to an 'exit' notification from the LSP client.

async fn on_exit(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> { Ok(()) }

/// Initializes and stores in state the instances that are based on the reported client capabilities.

async fn initialize_client_dependant_state(
	state: Arc<AsyncRwLock<State>>,
	workspace_folders: Option<Vec<WorkspaceFolder>>,
	window_capabilities: Option<WindowClientCapabilities>
) {

	let mut state = state.write().await;

	let analyzer = state.analyzer.clone();

	let file_system = state.file_system.clone();

	state.set_workspaces(WorkspaceManager::new(file_system, workspace_folders, analyzer));

	let request_manager = state.request_manager.clone();

	let work_done_supported =
		window_capabilities.map_or(false, |value| value.work_done_progress.map_or(false, |value| value));

	state.set_progress(ProgressManager::new(request_manager, work_done_supported));
}

/// Creates an initialized [`InitializeResult`] instance that describes the capabilities of the P4 Analyzer.

fn create_initialize_result(include_workspace_folders: bool) -> InitializeResult {

	let workspace = if include_workspace_folders {

		Some(WorkspaceServerCapabilities {
			workspace_folders: Some(WorkspaceFoldersServerCapabilities {
				supported: Some(true),
				change_notifications: Some(OneOf::Left(true))
			}),
			..Default::default()
		})
	}
	else {

		None
	};

	InitializeResult {
		capabilities: ServerCapabilities {
			text_document_sync: Some(TextDocumentSyncCapability::Options(TextDocumentSyncOptions {
				save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
					// FIXME: temporary solution
					include_text: Some(true)
				})),
				change: Some(TextDocumentSyncKind::INCREMENTAL),
				..Default::default()
			})),
			workspace,
			completion_provider: Some(CompletionOptions {
				resolve_provider: Some(true),
				trigger_characters: Some(vec!["(".to_string(), "<".to_string(), ".".to_string()]),
				all_commit_characters: None,
				work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None }
			}),
			hover_provider: Some(HoverProviderCapability::Simple(true)),
			signature_help_provider: Some(SignatureHelpOptions {
				trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
				retrigger_characters: None,
				work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None }
			}),
			declaration_provider: Some(DeclarationCapability::Simple(true)),
			definition_provider: Some(OneOf::Left(true)),
			type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
			implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
			references_provider: Some(OneOf::Left(true)),
			..Default::default()
		},
		server_info: Some(ServerInfo { name: String::from("P4 Analyzer"), version: Some(String::from("0.0.0")) })
	}
}
