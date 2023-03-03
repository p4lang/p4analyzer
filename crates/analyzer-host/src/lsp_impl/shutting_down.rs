use std::sync::Arc;
use async_rwlock::RwLock as AsyncRwLock;
use analyzer_abstractions::lsp_types::notification::Exit;
use crate::{lsp::{
	dispatch::Dispatch, dispatch_target::HandlerResult, state::LspServerState, DispatchBuilder,
}, json_rpc::ErrorCode};
use super::state::State;

/// Builds and then returns a dispatcher handling the [`LspServerState::ShuttingDown`] state.
pub(crate) fn create_dispatcher() -> Box<dyn Dispatch<State> + Send + Sync + 'static> {
	Box::new(
		DispatchBuilder::<State>::new(LspServerState::ShuttingDown)
			.for_unhandled_requests((ErrorCode::InvalidRequest, "The server is currently shutting down."))
			.for_notification_with_options::<Exit, _>(on_exit, |mut options| {
				options.transition_to(LspServerState::Stopped)
			})
			.build(),
	)
}

/// Responds to an 'exit' notification from the LSP client.
async fn on_exit(_: LspServerState, _: (), _: Arc<AsyncRwLock<State>>) -> HandlerResult<()> {
	Ok(())
}
