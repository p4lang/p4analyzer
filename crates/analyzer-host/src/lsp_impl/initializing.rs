use async_rwlock::RwLock as AsyncRwLock;
use std::sync::Arc;

use analyzer_abstractions::lsp_types::{
	notification::{Exit, Initialized},
	request::RegisterCapability,
	DidChangeWatchedFilesRegistrationOptions, FileSystemWatcher, InitializedParams, Registration, RegistrationParams,
};

use crate::{
	fsm::LspServerStateDispatcher,
	json_rpc::{to_json, ErrorCode},
	lsp::{
		dispatch::Dispatch,
		dispatch_target::{HandlerError, HandlerResult},
		state::LspServerState,
		DispatchBuilder, RELATIVE_P4_SOURCEFILES_GLOBPATTERN,
	},
};

use super::state::State;

/// Builds and then returns a dispatcher handling the [`LspServerState::Initializing`] state.
pub(crate) fn create_dispatcher() -> LspServerStateDispatcher {
	Box::new(
		DispatchBuilder::<State>::new(LspServerState::Initializing)
			.for_notification_with_options::<Initialized, _>(on_client_initialized, |mut options| {
				options.transition_to(LspServerState::ActiveInitialized)
			})
			.for_unhandled_requests((ErrorCode::ServerNotInitialized, "The server is initializing."))
			.for_notification_with_options::<Exit, _>(on_exit, |mut options| {
				options.transition_to(LspServerState::Stopped)
			})
			.build(),
	)
}

/// Responds to an `'initialized'` notification from the LSP client.
///
/// Once the client and server are initialized, the server will dynamically register for watched `.p4` files in any
/// opened workspaces.
async fn on_client_initialized(
	_: LspServerState,
	_: InitializedParams,
	state: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	let state = state.read().await;

	// If the server has been started without any workspace context, then simply return.
	if !state.has_workspaces() {
		return Ok(());
	}

	let registration_params = RegistrationParams {
		registrations: vec![Registration {
			id: "p4-analyzer-watched-files".to_string(),
			method: "workspace/didChangeWatchedFiles".to_string(),
			register_options: Some(
				to_json(DidChangeWatchedFilesRegistrationOptions {
					watchers: vec![FileSystemWatcher {
						glob_pattern: RELATIVE_P4_SOURCEFILES_GLOBPATTERN.into(),
						kind: None, // Default to create | change | delete.
					}],
				})
				.unwrap(),
			),
		}],
	};

	if let Err(_) = state.request_manager.send::<RegisterCapability>(registration_params).await {
		return Err(HandlerError::new("Error registering dynamic capability for 'workspace/didChangeWatchedFiles'."));
	}

	state.workspaces().index(state.progress_manager()).await; // Index the workspace folders.

	Ok(())
}

/// Responds to an 'exit' notification from the LSP client.
async fn on_exit(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> { Ok(()) }
