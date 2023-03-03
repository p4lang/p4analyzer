use std::sync::Arc;
use async_rwlock::RwLock as AsyncRwLock;

use analyzer_abstractions::lsp_types::{
	notification::{Exit, Initialized},
	InitializedParams,
};

use crate::{lsp::{
	dispatch::Dispatch, dispatch_target::HandlerResult, state::LspServerState, DispatchBuilder,
}, json_rpc::ErrorCode};

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

/// Responds to a 'initialized' notification from the LSP client.
async fn on_client_initialized(
	_: LspServerState,
	_: InitializedParams,
	_: Arc<AsyncRwLock<State>>,
) -> HandlerResult<()> {
	Ok(())
}

/// Responds to an 'exit' notification from the LSP client.
async fn on_exit(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> {
	Ok(())
}
