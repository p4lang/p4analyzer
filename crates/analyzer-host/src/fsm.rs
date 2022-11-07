use crate::json_rpc::{from_json, message::*, DeserializeError, ErrorCode};
use analyzer_abstractions::{lsp_types::InitializeParams, LoggerImpl};
use thiserror::Error;

/// Represents the valid states of a [`ProtocolMachine`].
#[derive(Debug, PartialEq, Copy, Clone)]
pub(crate) enum ProtocolState {
	/// The machine is currently processing an initialization request from the client.
	Initializing,

	/// The machine is active, but has not yet received an initialization request from the client.
	ActiveUninitialized,

	/// The machine is active and ready to process requests from the client.
	ActiveInitialized,

	/// The machine is currently processing a shutdown request from the client.
	ShuttingDown,

	/// The machine has shutdown and will no longer process requests from the client.
	Stopped,
}

/// Represents an error in protocol while processing a received client message.
#[derive(Error, Debug)]
pub enum ProtocolError {
	/// The received request was not expected.
	#[error("The received request was not expected.")]
	UnexpectedRequest,

	/// The received request was was malformed or invalid
	#[error("The received request was was malformed or invalid.")]
	BadRequest(#[from] DeserializeError),
}

/// A state machine that models the Language Server Protocol (LSP). In the specification, a LSP server has a lifecycle
/// that is managed fully by the client. [`ProtocolMachine`] ensures that the server responds accordingly by
/// transitioning itself through states based on the requests received, and then processed on behalf of the client. If
/// the server is in an invalid state for a given request, then the client will receive an apprpriate error response.
#[derive(Copy, Clone)]
pub(crate) struct ProtocolMachine<'machine> {
	/// A logger that the [`ProtocolMachine`] will use to output log messages.
	logger: &'machine LoggerImpl,

	/// The current [`ProtocolState`].
	pub(crate) current_state: ProtocolState,
}

impl<'machine> ProtocolMachine<'machine> {
	/// Initializes a new [`ProtocolMachine`] that will start in the [`ProtocolState::ActiveUninitialized`] state.
	pub fn new(logger: &'machine LoggerImpl) -> Self {
		ProtocolMachine {
			logger,
			current_state: ProtocolState::ActiveUninitialized,
		}
	}

	/// Returns `true` if the current [`ProtocolMachine`] is in an active state; otherwise `false`.
	pub fn is_active(&self) -> bool {
		self.current_state != ProtocolState::Stopped
	}

	/// Processes a [`Message`] for the current [`ProtocolState`], and returns an optional [`Message`] that represents its response.
	///
	/// If the supplied message yields a 'bad request' (i.e., it contains malformed or invalid parameter data), then the
	/// [`ProtocolMachine`] will transition back to the state it was in prior to processing the supplied message.
	pub async fn process_request(
		&mut self,
		message: Message,
	) -> Result<Option<Message>, ProtocolError> {
		match self.current_state {
			ProtocolState::Initializing => self.on_initializing(message).await,
			ProtocolState::ActiveUninitialized => self.on_active_uninitialized(message).await,
			ProtocolState::ActiveInitialized => Err(ProtocolError::UnexpectedRequest),
			ProtocolState::ShuttingDown => Err(ProtocolError::UnexpectedRequest),
			ProtocolState::Stopped => Err(ProtocolError::UnexpectedRequest),
		}
	}

	/// Message handling for the [`ProtocolState::Initializing`] state.
	async fn on_initializing(
		&mut self,
		message: Message,
	) -> Result<Option<Message>, ProtocolError> {
		match message {
			Message::Notification(notification) if notification.is_exit() => {
				self.transition_to(ProtocolState::Stopped);
				Ok(None)
			}

			// Reject any other request.
			Message::Request(request) => {
				let response = Response::new_error(
					request.id,
					ErrorCode::ServerNotInitialized as i32,
					"The server is currently initializing.",
				);

				Ok(Some(Message::Response(response)))
			}

			// Ignore any other notifications.
			Message::Notification(_) => Ok(None),

			_ => Err(ProtocolError::UnexpectedRequest),
		}
	}

	/// Message handling for the [`ProtocolState::ActiveUninitialized`] state.
	async fn on_active_uninitialized(
		&mut self,
		message: Message,
	) -> Result<Option<Message>, ProtocolError> {
		match message {
			// Process an 'exit' notification by immediately transitioning to 'stopped'.
			Message::Notification(notification) if notification.is_exit() => {
				self.transition_to(ProtocolState::Stopped);
				Ok(None)
			}

			// Process an 'initialize' request.
			Message::Request(request) if request.is_initialize() => {
				let params = from_json::<InitializeParams>("InitializeParams", &request.params)?;

				self.transition_to(ProtocolState::Initializing);

				let data = "Hello";

				self.transition_to(ProtocolState::ActiveInitialized);

				Ok(Some(Message::Response(Response::new(request.id, data))))
			}

			// Reject any other request.
			Message::Request(request) => {
				let response = Response::new_error(
					request.id,
					ErrorCode::ServerNotInitialized as i32,
					"An 'initialize' request is required.",
				);

				Ok(Some(Message::Response(response)))
			}

			// Ignore any other notifications.
			Message::Notification(_) => Ok(None),

			_ => Err(ProtocolError::UnexpectedRequest),
		}
	}

	/// Transitions the current [`ProtocolMachine`] to a new state.
	///
	/// Once transitioned, the machine will begin processing requests from the client appropriately to that state.
	fn transition_to(&mut self, target_state: ProtocolState) {
		self.current_state = target_state;
	}
}
