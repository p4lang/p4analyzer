use analyzer_abstractions::{async_trait::async_trait, tracing::info};
use async_rwlock::RwLock as AsyncRwLock;
use std::{
	collections::HashMap,
	sync::{Arc, RwLock}
};

use crate::json_rpc::{
	message::{Message, Response},
	ErrorCode
};

use super::{
	state::{LspServerState, LspTransitionTarget},
	LspProtocolError
};

use dyn_clonable::*;

/// Processes a [`Message`].
#[async_trait]
#[clonable]

pub(crate) trait DispatchTarget<TState: Send + Sync>: Clone {
	/// Processes a [`Message`] and returns a tuple of an optional response, and a target [`LspServerState`] from which
	/// further messages should be processed.
	///
	/// An instance of [`TState`] may be mutated by the [`DispatchTarget`] during the processing of `message`.

	async fn process_message(
		&self,
		current_state: LspServerState,
		message: Arc<Message>,
		state: Arc<AsyncRwLock<TState>>
	) -> Result<(Option<Message>, LspServerState), LspProtocolError>;

	/// Captures the desired [`LspServerState`] if the received message was successfully processed.

	fn set_transition_target(&mut self, target: LspTransitionTarget);
}

pub(crate) type AnyDispatchTarget<TState> = Box<dyn DispatchTarget<TState> + Send + Sync>;

/// Dispatches [`Message`] instances to underlying [`DispatchTarget`] implementations.
#[async_trait]
#[clonable]

pub(crate) trait Dispatch<TState: Send + Sync>: Clone {
	/// Processes a [`Message`] and returns a tuple of an optional response, and a target [`LspServerState`] from which
	/// further messages should be processed.
	///
	/// An instance of [`TState`] may be mutated by the [`DispatchTarget`] implementation during the processing of `message`.

	async fn dispatch(
		&self,
		message: Arc<Message>,
		state: Arc<AsyncRwLock<TState>>
	) -> Result<(Option<Message>, LspServerState), LspProtocolError>;
}

/// Provides a default [`Dispatch`] implementation.
#[derive(Clone)]

pub(crate) struct DefaultDispatch<TState: Send + Sync> {
	state: LspServerState,
	request_handlers: Arc<RwLock<HashMap<String, AnyDispatchTarget<TState>>>>,
	notification_handlers: Arc<RwLock<HashMap<String, AnyDispatchTarget<TState>>>>,
	missing_handler_error: Option<(ErrorCode, &'static str)>
}

impl<TState: Send + Sync> DefaultDispatch<TState> {
	/// Initializes a new [`DefaultDispatch`] instance for a given [`LspServerState`], and a collection of [`DispatchTarget`]
	/// implementations to consider when processing messages.

	pub fn new(
		state: LspServerState,
		request_handlers: Arc<RwLock<HashMap<String, AnyDispatchTarget<TState>>>>,
		notification_handlers: Arc<RwLock<HashMap<String, AnyDispatchTarget<TState>>>>,
		missing_handler_error: Option<(ErrorCode, &'static str)>
	) -> Self {

		Self { state, request_handlers, notification_handlers, missing_handler_error }
	}

	/// Retrieves the [`DispatchTarget`] that is registered to process the given message.
	///
	/// Returns `None` if no handler was registered.

	fn get_handler(&self, message: &Message) -> Option<AnyDispatchTarget<TState>> {

		let handlers = match message {
			Message::Request(request) => Some((self.request_handlers.read().unwrap(), &request.method)),
			Message::Notification(notification) => {
				Some((self.notification_handlers.read().unwrap(), &notification.method))
			}
			_ => None
		};

		match handlers {
			Some((handlers, method)) => handlers.get(method).cloned(),
			None => None
		}
	}
}

#[async_trait]

impl<TState: Clone + Send + Sync + 'static> Dispatch<TState> for DefaultDispatch<TState> {
	/// Processes a [`Message`] and returns a tuple of an optional response, and a target [`LspServerState`] from which
	/// further messages should be processed.
	///
	/// An instance of [`TState`] may be mutated by the [`DispatchTarget`] implementation during the processing of `message`.
	///
	/// If no handler could be found for `message`, then either a default 'error' response will be returned for the unhandled
	/// message, or an [`LspProtocolError::UnexpectedRequest`] error, depending on whether defaults have been set on the [`Dispatch`].

	async fn dispatch(
		&self,
		message: Arc<Message>,
		state: Arc<AsyncRwLock<TState>>
	) -> Result<(Option<Message>, LspServerState), LspProtocolError> {

		match self.get_handler(&*message) {
			Some(handler) => handler.process_message(self.state, message, state).await,
			None => {

				// If we have no handler for the message, then return either a response representing the default
				// error message for requests, or an 'unexpected request' error for anything else.
				match &*message {
					Message::Request(request) => {

						if let Some((code, message)) = &self.missing_handler_error {

							info!(
								error_code = *code as i32,
								custom_message = message,
								"Received an unexpected request ('{}'). Sending customized error.",
								request.method
							);

							return Ok((
								Some(Message::Response(Response::new_error(request.id.clone(), *code as i32, message))),
								self.state
							));
						}

						info!(
							"Received an unexpected request ('{}'). Sending internal server error. Missing handler?",
							request.method
						);

						Ok((
							Some(Message::Response(Response::new_error(
								request.id.clone(),
								ErrorCode::InternalError as i32,
								"Internal server error."
							))),
							self.state
						))
					}
					Message::Notification(notification) => {

						info!("Received an unexpected notification ('{}'). Missing handler?", notification.method);

						Err(LspProtocolError::UnexpectedRequest)
					}
					_ => Err(LspProtocolError::UnexpectedRequest)
				}
			}
		}
	}
}
