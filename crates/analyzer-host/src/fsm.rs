use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
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
	dispatchers: Arc<RwLock<HashMap<LspServerState, Arc<LspServerStateDispatcher>>>>,
	current_state: LspServerState,
	state: Arc<AsyncRwLock<State>>,
}

impl LspProtocolMachine {
	#[allow(clippy::needless_update)] // TODO: Remove this when new fields are added to the  `State` type.
	/// Initializes a new [`LspProtocolMachine`] that will start in its initial state.
	pub fn new(trace_value: Option<TraceValueAccessor>) -> Self {
		let dispatchers = Arc::new(RwLock::new(LspProtocolMachine::create_dispatchers()));

		Self {
			dispatchers,
			current_state: LSP_STARTING_STATE,
			state: Arc::new(AsyncRwLock::new(State {
				trace_value,
				..Default::default()
			})),
		}
	}

	/// Returns `true` if the current [`LspProtocolMachine`] is in an active state; otherwise `false`.
	pub fn is_active(&self) -> bool {
		self.current_state != LspServerState::Stopped
	}

	/// Processes a [`Message`] for the current [`LspProtocolMachine`], and returns an optional [`Message`] that represents
	/// its response.
	pub async fn process_message(
		&mut self,
		message: &Message,
	) -> Result<Option<Message>, LspProtocolError> {
		// let mut dispatchers = self.dispatchers;

		let current_state = self.current_state;
		let current_dispatcher = self.get_dispatcher(current_state); //self.dispatchers.entry(current_state).or_insert_with(|| LspProtocolMachine::default_dispatcher(current_state));

		match current_dispatcher.dispatch(message, self.state.clone()).await {
			Ok((response, next_state)) => {
				self.current_state = next_state;

				Ok(response)
			}
			Err(err) => Err(err),
		}
	}

	fn create_dispatchers() -> HashMap<LspServerState, Arc<LspServerStateDispatcher>> {
		[
			(LspServerState::ActiveUninitialized, Arc::new(create_dispatcher_active_uninitialized())),
			(LspServerState::Initializing, Arc::new(create_dispatcher_initializing())),
			(LspServerState::ActiveInitialized, Arc::new(create_dispatcher_active_initialized())),
			(LspServerState::ShuttingDown, Arc::new(create_dispatcher_shutting_down())),
			(LspServerState::Stopped, Arc::new(create_dispatcher_stopped())),
		].into()
	}

	fn get_dispatcher(&self, state: LspServerState) -> Arc<LspServerStateDispatcher> {
		let mut dispatchers = self.dispatchers.write().unwrap();

		let a = dispatchers.entry(state).or_insert_with(|| LspProtocolMachine::default_dispatcher(state));

		a.clone()
	}

	fn default_dispatcher(state: LspServerState) -> Arc<LspServerStateDispatcher> {
		Arc::new(Box::new(
			DispatchBuilder::new(state)
				.for_unhandled_requests((
					ErrorCode::InternalError,
					"The server is in an unsupported state.",
				))
				.build(),
		))
	}
}
