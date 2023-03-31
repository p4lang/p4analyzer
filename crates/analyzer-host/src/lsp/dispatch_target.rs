use core::fmt::Debug;
use std::{
	future::Future,
	pin::Pin,
	sync::Arc,
};
use async_rwlock::RwLock as AsyncRwLock;
use analyzer_abstractions::{async_trait::async_trait, tracing::error};
use serde::{de::DeserializeOwned, Serialize};

use crate::json_rpc::{
	from_json,
	message::{Message, Response},
	ErrorCode,
};

use super::{
	dispatch::DispatchTarget,
	state::{LspServerState, LspTransitionTarget},
	LspProtocolError,
};

use dyn_clonable::*;

/// An error that can be produced when processing a message.
#[derive(Clone)]
pub(crate) struct HandlerError {
	message: &'static str,

	#[allow(dead_code)]
	data: Option<serde_json::Value>,
}

impl HandlerError {
	#[allow(dead_code)]
	/// Initializes a new [`HandlerError`] with a given error message.
	pub(crate) fn new(message: &'static str) -> Self {
		Self {
			message,
			data: None,
		}
	}

	#[allow(dead_code)]
	/// Initializes a new [`HandlerError`] with a given error message and serializable data.
	pub(crate) fn new_with_data<TData: Serialize>(
		message: &'static str,
		data: Option<TData>,
	) -> Self {
		Self {
			message,
			data: data.map(|v| serde_json::to_value(v).unwrap()),
		}
	}
}

/// A result type that represents success or failure ([`HandlerError`]).
pub(crate) type HandlerResult<T> = Result<T, HandlerError>;

pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'static>>;

/// An asynchronous request handler.
///
/// A request handler receives some deserialized parameters, and returns a [`HandlerRequest`]. During execution, the
/// request handler can also mutate an instance of [`TState`].
#[clonable]
pub(crate) trait AsyncRequestHandlerFn<TState, TParams, TResult>: Clone
where
	TState: Send + Sync
{
	/// Invokes the request handler, returning a future that will yield a [`HandlerResult`].
	fn call(
		&self,
		current_state: LspServerState,
		params: TParams,
		state: Arc<AsyncRwLock<TState>>,
	) -> BoxFuture<HandlerResult<TResult>>;
}

impl<TState, TParams, TResult, T, F> AsyncRequestHandlerFn<TState, TParams, TResult> for T
where
	TState: Send + Sync + 'static,
	T: (Fn(LspServerState, TParams, Arc<AsyncRwLock<TState>>) -> F) + Clone + Send + Sync + 'static,
	F: Future<Output = HandlerResult<TResult>> + Send + Sync + 'static,
{
	fn call(
		&self,
		current_state: LspServerState,
		params: TParams,
		state: Arc<AsyncRwLock<TState>>,
	) -> BoxFuture<HandlerResult<TResult>> {
		Box::pin(self(current_state, params, state))
	}
}

/// Represents any asynchronous request handler.
pub(crate) type AnyAsyncRequestHandlerFn<TState, TParams, TResult> =
	Box<dyn (AsyncRequestHandlerFn<TState, TParams, TResult>) + Send + Sync>;

/// Processes a message that represents a request.
#[derive(Clone)]
pub(crate) struct RequestDispatchTarget<TState, TParams, TResult>
where
	TState: Clone + Send + Sync
{
	pub handler_fn: AnyAsyncRequestHandlerFn<TState, TParams, TResult>,
	pub transition_target: LspTransitionTarget,
}

impl<TState, TParams, TResult> RequestDispatchTarget<TState, TParams, TResult>
where
	TState: Clone + Send + Sync,
	TParams: DeserializeOwned + Send + Debug,
	TResult: Serialize + Send,
{
	/// Initializes a new [`RequestDispatchTarget`] for a given handler function.
	pub fn new(handler_fn: Box<dyn (AsyncRequestHandlerFn<TState, TParams, TResult>) + Send + Sync>) -> Self {
		Self {
			handler_fn,
			transition_target: LspTransitionTarget::Current,
		}
	}

	fn next_state(&self, current_state: LspServerState) -> LspServerState {
		match self.transition_target {
			LspTransitionTarget::Next(next_state) => next_state,
			LspTransitionTarget::Current => current_state,
		}
	}
}

#[async_trait]
impl<TState, TParams, TResult> DispatchTarget<TState> for RequestDispatchTarget<TState, TParams, TResult>
where
	TState: Clone + Send + Sync + 'static,
	TParams: DeserializeOwned + Clone + Send + Debug + 'static,
	TResult: Serialize + Clone + Send + 'static,
{
	async fn process_message(
		&self,
		current_state: LspServerState,
		message: Arc<Message>,
		state: Arc<AsyncRwLock<TState>>,
	) -> Result<(Option<Message>, LspServerState), LspProtocolError> {
		match &*message {
			Message::Request(request) => {
				let method = request.method.as_str();
				let params = from_json::<TParams>(&request.params)?;
				let (response, next_state) =
					match self.handler_fn.call(current_state, params, state).await {
						Ok(result) => (
							Response::new(request.id.clone(), result),
							self.next_state(current_state),
						),
						Err(err) => {
							error!(
								method = method,
								"Error processing request '{}': {}", method, err.message
							);

							(
								Response::new_error(
									request.id.clone(),
									ErrorCode::InternalError as i32,
									err.message,
								),
								current_state,
							)
						}
					};

				Ok((Some(Message::Response(response)), next_state))
			}
			_ => Err(LspProtocolError::UnexpectedRequest),
		}
	}

	fn set_transition_target(&mut self, target: LspTransitionTarget) {
		self.transition_target = target;
	}
}

/// Processes a message that represents a notification.
#[derive(Clone)]
pub(crate) struct NotificationDispatchTarget<TState, TParams>
where
	TState: Send + Sync
{
	pub handler_fn: AnyAsyncRequestHandlerFn<TState, TParams, ()>,
	pub transition_target: LspTransitionTarget,
}

impl<TState, TParams> NotificationDispatchTarget<TState, TParams>
where
	TState: Send + Sync,
	TParams: DeserializeOwned + Send + Debug,
{
	/// Initializes a new [`NotificationDispatchTarget`] for a given handler function.
	pub fn new(handler_fn: Box<dyn (AsyncRequestHandlerFn<TState, TParams, ()>) + Send + Sync>) -> Self {
		Self {
			handler_fn,
			transition_target: LspTransitionTarget::Current,
		}
	}

	fn next_state(&self, current_state: LspServerState) -> LspServerState {
		match self.transition_target {
			LspTransitionTarget::Next(next_state) => next_state,
			LspTransitionTarget::Current => current_state,
		}
	}
}

#[async_trait]
impl<TState, TParams> DispatchTarget<TState> for NotificationDispatchTarget<TState, TParams>
where
	TState: Clone + Send + Sync + 'static,
	TParams: DeserializeOwned + Clone + Send + Debug + 'static,
{
	async fn process_message(
		&self,
		current_state: LspServerState,
		message: Arc<Message>,
		state: Arc<AsyncRwLock<TState>>,
	) -> Result<(Option<Message>, LspServerState), LspProtocolError> {
		match &*message {
			Message::Notification(request) => {
				let method = request.method.as_str();
				let params = from_json::<TParams>(&request.params)?;

				if let Err(err) = self.handler_fn.call(current_state, params, state).await {
					error!(
						method = request.method.as_str(),
						"Error processing notification '{}': {}", method, err.message
					);

					return Ok((None, current_state));
				}

				Ok((None, self.next_state(current_state)))
			}
			_ => Err(LspProtocolError::UnexpectedRequest),
		}
	}

	fn set_transition_target(&mut self, target: LspTransitionTarget) {
		self.transition_target = target;
	}
}
