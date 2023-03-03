use crate::{lsp::{
	dispatch::Dispatch, state::LspServerState, DispatchBuilder,
}, json_rpc::ErrorCode};

use super::state::State;

/// Builds and then returns a dispatcher handling the [`LspServerState::Stopped`] state.
pub(crate) fn create_dispatcher() -> Box<dyn Dispatch<State> + Send + Sync + 'static> {
	Box::new(
		DispatchBuilder::<State>::new(LspServerState::Stopped)
			.for_unhandled_requests((ErrorCode::InvalidRequest, "The server has stopped."))
			.build(),
	)
}
