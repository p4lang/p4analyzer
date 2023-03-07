use analyzer_abstractions::{
	lsp_types::{notification::Notification, request::Request},
	tracing::error,
};
use async_channel::SendError;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use std::{
	collections::HashMap,
	fmt,
	sync::{Arc, RwLock},
};

use crate::json_rpc::{DeserializeError, ErrorCode, message::Message};

use self::{state::{LspServerState}, fluent::state::TransitionBuilder, dispatch::{DefaultDispatch, AnyDispatchTarget, Dispatch}, dispatch_target::{AsyncRequestHandlerFn, RequestDispatchTarget, NotificationDispatchTarget}};

pub(crate) mod fluent;
pub(crate) mod state;
pub(crate) mod dispatch;
pub(crate) mod dispatch_target;
pub(crate) mod request;

/// Represents an error in protocol while processing a received client message.
#[derive(Error, Debug, Clone, Copy)]
pub enum LspProtocolError {
	/// The received request was not expected.
	#[error("The received request was not expected.")]
	UnexpectedRequest,

	#[error("The received repsonse was not expected.")]
	UnexpectedResponse,

	/// The message was malformed or invalid.
	#[error("The message was malformed or invalid.")]
	BadRequest(#[from] DeserializeError),

	#[error("There was an error sending or receiving a message.")]
	TransportError,
}

/// Provides a fluent API for building [`Dispatch`] implementations.
pub(crate) struct DispatchBuilder<TState>
where
	TState: Send + Sync
{
	state: LspServerState,
	request_handlers: Arc<RwLock<HashMap<String, Arc<AnyDispatchTarget<TState>>>>>,
	notification_handlers: Arc<RwLock<HashMap<String, Arc<AnyDispatchTarget<TState>>>>>,
	missing_handler_error: Option<(ErrorCode, &'static str)>,
}

impl<TState> DispatchBuilder<TState>
where
	TState: Send + Sync + 'static
{
	/// Initializes a new [`DispatchBuilder`] for a given [`LspServerState`].
	pub fn new(state: LspServerState) -> Self {
		Self {
			state,
			request_handlers: Arc::new(RwLock::new(HashMap::new())),
			notification_handlers: Arc::new(RwLock::new(HashMap::new())),
			missing_handler_error: None,
		}
	}

	/// Registers a handler for a given type of request message.
	pub fn for_request<T, THandler>(&mut self, handler: THandler) -> &mut Self
	where
		T: Request + 'static,
		T::Params: Clone + DeserializeOwned + Send + fmt::Debug,
		T::Result: Clone + Serialize + Send,
		THandler: AsyncRequestHandlerFn<TState, T::Params, T::Result> + Send + Sync + 'static
	{
		let target = RequestDispatchTarget::<TState, T::Params, T::Result>::new(Box::new(handler));

		// target.handler_fn = Some(Box::new(handler));

		self.request_handlers.write().unwrap().insert(String::from(T::METHOD), Arc::new(Box::new(target)));

		self
	}

	/// Registers a handler for a given type of request message, and supports additional options to apply
	/// during its registration.
	pub fn for_request_with_options<T, THandler>(&mut self, handler: THandler, request_builder: fn(TransitionBuilder<TState>) -> ()) -> &mut Self
	where
		T: Request + 'static,
		T::Params: Clone + DeserializeOwned + Send + fmt::Debug,
		T::Result: Clone + Serialize + Send,
		THandler: AsyncRequestHandlerFn<TState, T::Params, T::Result> + Send + Sync + 'static
	{
		let mut target = RequestDispatchTarget::<TState, T::Params, T::Result>::new(Box::new(handler));

		// target.handler_fn = Some(Box::new(handler));
		request_builder(TransitionBuilder::new(&mut target));

		self.request_handlers.write().unwrap().insert(String::from(T::METHOD), Arc::new(Box::new(target)));

		self
	}

	pub fn for_unhandled_requests(&mut self, error: (ErrorCode, &'static str)) -> &mut Self {
		self.missing_handler_error = Some(error);

		self
	}

	/// Registers a handler for a given type of notification message.
	pub fn for_notification<T, THandler>(&mut self, handler: THandler) -> &mut Self
	where
		T: Notification + 'static,
		T::Params: Clone + DeserializeOwned + Send + fmt::Debug,
		THandler: AsyncRequestHandlerFn<TState, T::Params, ()> + Send + Sync + 'static
	{
		let target = NotificationDispatchTarget::<TState, T::Params>::new(Box::new(handler));

		// target.handler_fn = Some(Box::new(handler));

		self.notification_handlers.write().unwrap().insert(String::from(T::METHOD), Arc::new(Box::new(target)));

		self
	}

	/// Registers a handler for a given type of notification message, and supports additional options to apply
	/// during its registration.
	pub fn for_notification_with_options<T, THandler>(&mut self, handler: THandler, request_builder: fn(TransitionBuilder<TState>) -> ()) -> &mut Self
	where
		T: Notification + 'static,
		T::Params: Clone + DeserializeOwned + Send + fmt::Debug,
		THandler: AsyncRequestHandlerFn<TState, T::Params, ()> + Send + Sync + 'static
	{
		let mut target = NotificationDispatchTarget::<TState, T::Params>::new(Box::new(handler));

		// target.handler_fn = Some(Box::new(handler));
		request_builder(TransitionBuilder::new(&mut target));

		self.notification_handlers.write().unwrap().insert(String::from(T::METHOD), Arc::new(Box::new(target)));

		self
	}

	/// Builds the [`Dispatch`] implementation for the current set of handler registrations.
	pub fn build(&self) -> impl Dispatch<TState> {
		DefaultDispatch::new(self.state, self.request_handlers.clone(), self.notification_handlers.clone(), self.missing_handler_error)
	}
}
