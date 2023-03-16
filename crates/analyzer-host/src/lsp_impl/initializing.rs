use std::sync::Arc;
use async_rwlock::RwLock as AsyncRwLock;

use analyzer_abstractions::{lsp_types::{
	notification::{Exit, Initialized},
	InitializedParams, request::RegisterCapability, RegistrationParams, Registration, DidChangeWatchedFilesRegistrationOptions, FileSystemWatcher,
}, tracing::{event_enabled, Level, info}};

use crate::{lsp::{
	dispatch::Dispatch, dispatch_target::{HandlerResult, HandlerError}, state::LspServerState, DispatchBuilder,
}, json_rpc::{ErrorCode, to_json}};

use super::state::State;

/// Builds and then returns a dispatcher handling the [`LspServerState::Initializing`] state.
pub(crate) fn create_dispatcher() -> Box<dyn Dispatch<State> + Send + Sync + 'static> {
	Box::new(
		DispatchBuilder::<State>::new(LspServerState::Initializing)
			.for_notification_with_options::<Initialized, _>(
				on_client_initialized,
				|mut options| options.transition_to(LspServerState::ActiveInitialized),
			)
			.for_unhandled_requests((ErrorCode::ServerNotInitialized, "The server is waiting on the client to send the 'initialized' notification."))
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
	state: Arc<AsyncRwLock<State>>
) -> HandlerResult<()>
{
	let state = state.read().await;

	// If the server has been started without any workspace context, then simply return.
	if !state.has_workspaces() {
		return Ok(());
	}

	let registration_params = RegistrationParams {
		registrations: vec![
			Registration {
				id: "p4-analyzer-watched-files".to_string(),
				method: "workspace/didChangeWatchedFiles".to_string(),
				register_options: Some(
					to_json(
						DidChangeWatchedFilesRegistrationOptions {
							watchers: vec![FileSystemWatcher {
								glob_pattern: "**/*.p4".to_string(),
								kind: None // Default to create | change | delete.
							}]
						}).unwrap())
			}
		]
	};

	if let Err(_) = state.request_manager.send::<RegisterCapability>(registration_params).await {
		return Err(HandlerError::new("Error registering dynamic capability for 'workspace/didChangeWatchedFiles'."));
	}

	if event_enabled!(Level::INFO) {
		let workspaces: Vec<String> = state.workspaces().into_iter().map(|(_, workspace)| { format!("{}", *workspace) }).collect();

		info!(workspaces = workspaces.join(", "), "Registered for workspace file changes.");
	}

	Ok(())
}

/// Responds to an 'exit' notification from the LSP client.
async fn on_exit(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> {
	Ok(())
}
