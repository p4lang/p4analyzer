use std::collections::HashMap;
use std::sync::Arc;
use async_rwlock::RwLock as AsyncRwLock;

use crate::json_rpc::message::Message;
use crate::json_rpc::ErrorCode;
use crate::lsp::dispatch::Dispatch;
use crate::lsp::{DispatchBuilder, LspProtocolError};
use crate::lsp_impl::active_initialized::create_dispatcher as create_dispatcher_active_initialized;
use crate::lsp_impl::active_uninitialized::create_dispatcher as create_dispatcher_active_uninitialized;
use crate::lsp_impl::initializing::create_dispatcher as create_dispatcher_initializing;
use crate::lsp_impl::shutting_down::create_dispatcher as create_dispatcher_shutting_down;
use crate::lsp_impl::stopped::create_dispatcher as create_dispatcher_stopped;
use crate::{lsp::state::LspServerState, lsp_impl::state::State, tracing::TraceValueAccessor};

/// The [`LspServerState`] that a [`LspProtocolMachine`] will initially start in.
const LSP_STARTING_STATE: LspServerState = LspServerState::ActiveUninitialized;

type LspServerStateDispatcher = Box<dyn (Dispatch<State>) + Send + Sync>;

/// A state machine that models the Language Server Protocol (LSP). In the specification, a LSP server has a lifecycle
/// that is managed fully by the client. [`LspProtocolMachine`] ensures that the server responds accordingly by
/// transitioning itself through states based on the requests received, and then processed on behalf of the client. If
/// the server is in an invalid state for a given request, then the client will receive an appropriate error response.
pub(crate) struct LspProtocolMachine {
	dispatchers: HashMap<LspServerState, LspServerStateDispatcher>,
	current: (LspServerState, LspServerStateDispatcher),
	state: Arc<AsyncRwLock<State>>,
}

impl LspProtocolMachine {
	#[allow(clippy::needless_update)] // TODO: Remove this when new fields are added to the  `State` type.
	/// Initializes a new [`LspProtocolMachine`] that will start in its initial state.
	pub fn new(trace_value: Option<TraceValueAccessor>) -> Self {
		let dispatchers = LspProtocolMachine::create_dispatchers();
		let current = dispatchers.get(&LSP_STARTING_STATE).map_or_else(
			|| LspProtocolMachine::default_dispatcher(LSP_STARTING_STATE),
			|v| v.clone(),
		);

		Self {
			dispatchers,
			current: (LSP_STARTING_STATE, current),
			state: Arc::new(AsyncRwLock::new(State {
				trace_value,
				..Default::default()
			})),
		}
	}

	/// Returns `true` if the current [`LspProtocolMachine`] is in an active state; otherwise `false`.
	pub fn is_active(&self) -> bool {
		let (current_state, _) = self.current;

		current_state != LspServerState::Stopped
	}

	/// Processes a [`Message`] for the current [`LspProtocolMachine`], and returns an optional [`Message`] that represents
	/// its response.
	pub async fn process_message(
		&mut self,
		message: &Message,
	) -> Result<Option<Message>, LspProtocolError> {
		let (current_state, current_dispatcher) = &self.current;

		match current_dispatcher
			.dispatch(message, self.state.clone())
			.await
		{
			Ok((response, next_state)) => {
				if *current_state != next_state {
					let next_dispatcher = self.dispatchers.get(&next_state).map_or_else(
						|| LspProtocolMachine::default_dispatcher(next_state),
						|v| v.clone(),
					);

					self.current = (next_state, next_dispatcher);
				}

				Ok(response)
			}
			Err(err) => Err(err),
		}
	}

	fn create_dispatchers() -> HashMap<LspServerState, LspServerStateDispatcher> {
		[
			(LspServerState::ActiveUninitialized, create_dispatcher_active_uninitialized()),
			(LspServerState::Initializing, create_dispatcher_initializing()),
			(LspServerState::ActiveInitialized, create_dispatcher_active_initialized()),
			(LspServerState::ShuttingDown, create_dispatcher_shutting_down()),
			(LspServerState::Stopped, create_dispatcher_stopped()),
		].into()
	}

	fn default_dispatcher(state: LspServerState) -> LspServerStateDispatcher {
		Box::new(
			DispatchBuilder::new(state)
				.for_unhandled_requests((
					ErrorCode::InternalError,
					"The server is in an unsupported state.",
				))
				.build(),
		)
	}
}
