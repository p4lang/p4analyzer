use std::sync::Arc;
use async_rwlock::RwLock as AsyncRwLock;

use analyzer_abstractions::{lsp_types::{
	notification::Exit, request::Initialize, CompletionOptions, DeclarationCapability,
	HoverProviderCapability, ImplementationProviderCapability, InitializeParams, InitializeResult,
	OneOf, ServerCapabilities, ServerInfo, SignatureHelpOptions, TextDocumentSyncCapability,
	TextDocumentSyncKind, TypeDefinitionProviderCapability, WorkDoneProgressOptions,
	TextDocumentSyncSaveOptions, TextDocumentSyncOptions, SaveOptions,
	TextDocumentSyncKind, TypeDefinitionProviderCapability, WorkDoneProgressOptions, WorkspaceServerCapabilities,
	WorkspaceFoldersServerCapabilities, WorkspaceFileOperationsServerCapabilities, FileOperationRegistrationOptions, FileOperationFilter,
	FileOperationPattern,
}, tracing::info};

use crate::{
	json_rpc::ErrorCode,
	lsp::{
		dispatch::Dispatch, dispatch_target::{HandlerResult, HandlerError}, state::LspServerState, DispatchBuilder,
	},
};

use super::state::State;

/// Builds and then returns a dispatcher handling the [`LspServerState::ActiveUninitialized`] state.
pub(crate) fn create_dispatcher() -> Box<dyn Dispatch<State> + Send + Sync + 'static> {
	Box::new(
		DispatchBuilder::<State>::new(LspServerState::ActiveUninitialized)
			.for_request_with_options::<Initialize, _>(on_initialize, |mut options| {
				options.transition_to(LspServerState::Initializing)
			})
			.for_unhandled_requests((ErrorCode::ServerNotInitialized, "An 'initialize' request is required."))
			.for_notification_with_options::<Exit, _>(on_exit, |mut options| {
				options.transition_to(LspServerState::Stopped)
			})
			.build(),
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
	_: Arc<AsyncRwLock<State>>,
) -> HandlerResult<InitializeResult> {
	match params.workspace_folders {
		Some(folders) => {
			info!("Initialized with 1 or more workspace folders");
			for folder in folders.into_iter() {
				info!("{:?}", folder);
			}
		},
		None => info!("Initialized with no workspace folders.")
	};

	match params.capabilities.workspace {
		Some(capabilities) if capabilities.did_change_watched_files != None => {
			let supports_workspace_folders = capabilities.workspace_folders.unwrap_or(false);

			Ok(create_initialize_result(supports_workspace_folders))
		},
		_ => Err(HandlerError::new("P4 Analyzer requires the Workspace Watched Files capability from the client."))
	}
}

/// Responds to an 'exit' notification from the LSP client.
async fn on_exit(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> {
	Ok(())
}

/// Creates an initialized [`InitializeResult`] instance that describes the capabilities of the P4 Analyzer.
fn create_initialize_result(supports_workspace_folders: bool) -> InitializeResult {
	let workspace =
		if supports_workspace_folders {
			Some(WorkspaceServerCapabilities {
				workspace_folders: Some(WorkspaceFoldersServerCapabilities {
					supported: Some(true),
					change_notifications: Some(OneOf::Left(true))
				}),
				// file_operations: Some(WorkspaceFileOperationsServerCapabilities {
				// 	did_create: Some(FileOperationRegistrationOptions { filters: vec![ FileOperationFilter {pattern: FileOperationPattern { glob: "**/*.rs".to_string() } } ] }),
				// 	..Default::default()
				// }),
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
			text_document_sync: Some(TextDocumentSyncCapability::Kind(
				TextDocumentSyncKind::INCREMENTAL,
			)),
			completion_provider: Some(CompletionOptions {
				resolve_provider: Some(true),
				trigger_characters: Some(vec!["(".to_string(), "<".to_string(), ".".to_string()]),
				all_commit_characters: None,
				work_done_progress_options: WorkDoneProgressOptions {
					work_done_progress: None,
				},
			}),
			hover_provider: Some(HoverProviderCapability::Simple(true)),
			signature_help_provider: Some(SignatureHelpOptions {
				trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
				retrigger_characters: None,
				work_done_progress_options: WorkDoneProgressOptions {
					work_done_progress: None,
				},
			}),
			declaration_provider: Some(DeclarationCapability::Simple(true)),
			definition_provider: Some(OneOf::Left(true)),
			type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
			implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
			references_provider: Some(OneOf::Left(true)),
			..Default::default()
		},
		server_info: Some(ServerInfo {
			name: String::from("p4-analyzer"),
			version: Some(String::from("0.0.0")),
		}),
	}
}
