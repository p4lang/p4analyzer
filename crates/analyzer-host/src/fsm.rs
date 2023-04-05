use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use analyzer_abstractions::fs::AnyEnumerableFileSystem;
use analyzer_abstractions::tracing::info;
use async_rwlock::RwLock as AsyncRwLock;

use crate::json_rpc::message::Message;
use crate::json_rpc::ErrorCode;
use crate::lsp::dispatch::Dispatch;
use crate::lsp::request::RequestManager;
use crate::lsp::{DispatchBuilder, LspProtocolError};
use crate::lsp_impl::active_initialized::create_dispatcher as create_dispatcher_active_initialized;
use crate::lsp_impl::active_uninitialized::create_dispatcher as create_dispatcher_active_uninitialized;
use crate::lsp_impl::initializing::create_dispatcher as create_dispatcher_initializing;
use crate::lsp_impl::shutting_down::create_dispatcher as create_dispatcher_shutting_down;
use crate::lsp_impl::stopped::create_dispatcher as create_dispatcher_stopped;
use crate::{lsp::state::LspServerState, lsp_impl::state::State, tracing::TraceValueAccessor};

/// The [`LspServerState`] that a [`LspProtocolMachine`] will initially start in.
const LSP_STARTING_STATE: LspServerState = LspServerState::ActiveUninitialized;

pub(crate) type LspServerStateDispatcher = Box<dyn (Dispatch<State>) + Send + Sync>;

/// A state machine that models the Language Server Protocol (LSP). In the specification, a LSP server has a lifecycle
/// that is managed fully by the client. [`LspProtocolMachine`] ensures that the server responds accordingly by
/// transitioning itself through states based on the requests received, and then processed on behalf of the client. If
/// the server is in an invalid state for a given request, then the client will receive an appropriate error response.
pub(crate) struct LspProtocolMachine {
	dispatchers: RwLock<HashMap<LspServerState, LspServerStateDispatcher>>,
	current_state: LspServerState,
	state: Arc<AsyncRwLock<State>>,
}

impl LspProtocolMachine {
	/// Initializes a new [`LspProtocolMachine`] that will start in its initial state.
	pub fn new(trace_value: Option<TraceValueAccessor>, request_manager: RequestManager, file_system: Arc<AnyEnumerableFileSystem>) -> Self {
		let dispatchers = RwLock::new(LspProtocolMachine::create_dispatchers());

		Self {
			dispatchers,
			current_state: LSP_STARTING_STATE,
			state: Arc::new(AsyncRwLock::new(State::new(trace_value, request_manager, file_system))),
		}
	}

	/// Returns `true` if the current [`LspProtocolMachine`] is in an active state; otherwise `false`.
	pub fn is_active(&self) -> bool {
		self.current_state != LspServerState::Stopped
	}

	/// Processes a [`Message`] for the current [`LspProtocolMachine`], and returns an optional [`Message`] that represents
	/// its response.
	pub async fn process_message(&mut self, message: Arc<Message>) -> Result<Option<Message>, LspProtocolError> {
		// Dispatch any received Request or Notification messages to the current Dispatcher.
		let current_state = self.current_state;
		let current_dispatcher = self.get_dispatcher(current_state);

		match current_dispatcher.dispatch(message, self.state.clone()).await {
			Ok((response, next_state)) => {
				if self.current_state != next_state {
					info!(current = format!("{:?}", self.current_state), next = format!("{:?}", next_state), "LspProtocolMachine state transition.");

					self.current_state = next_state;
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

	fn get_dispatcher(&self, state: LspServerState) -> LspServerStateDispatcher {
		let mut dispatchers = self.dispatchers.write().unwrap();

		let a = dispatchers.entry(state).or_insert_with(|| LspProtocolMachine::default_dispatcher(state));

		a.clone()
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
