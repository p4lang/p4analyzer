use core::fmt::Debug;
use std::{collections::HashMap, sync::{Arc, atomic::{AtomicI32, Ordering}}};
use async_rwlock::RwLock as AsyncRwLock;
use analyzer_abstractions::{futures_extensions::FutureCompletionSource, tracing::{error, info}};
use async_channel::{Sender, Receiver};
use cancellation::{OperationCanceled, CancellationToken};

use crate::{json_rpc::{message::{Message, Request}, RequestId, from_json}, MessageChannel};
use serde::{de::DeserializeOwned, Serialize};

use super::LspProtocolError;

type AnyFutureCompletionSource = FutureCompletionSource<Arc<Message>, LspProtocolError>;

/// Manages server side requests over a given [`MessageChannel`]. Requests will be sent via the
/// [`Sender`] element of the message channel, and responses will be awaited for over its [`Receiver`].
pub struct RequestManager {
	requests: Sender<Message>,
	responses: Receiver<Message>,
	request_id: AtomicI32,
	awaiting_requests: Arc<AsyncRwLock<HashMap<RequestId, Arc<AnyFutureCompletionSource>>>>,
}

impl RequestManager {
	/// Initializes a new [`RequestManager`] instance for a given message channel.
	pub fn new(message_channel: MessageChannel) -> Self {
		let (sender, receiver) = message_channel;

		Self {
			requests: sender,
			responses: receiver,
			request_id: AtomicI32::new(0),
			awaiting_requests: Arc::new(AsyncRwLock::new(HashMap::new())),
		}
	}

	/// Starts executing the [`RequestManager`] instance.
	///
	/// Once started, requests sent via the [`RequestManager::send`] method will be forwarded to the LSP client via
	/// the [`Sender`] element of the underlying message channel. Responses will then be read from the associated
	/// [`Receiver`] and matched with any awaiting requests.
	pub async fn start(&self, cancel_token: Arc<CancellationToken>) -> Result<(), OperationCanceled> {
		while !cancel_token.is_canceled() {
			match self.responses.recv().await {
				Ok(message) => {
					if cancel_token.is_canceled() {
						break;
					}

					if let Message::Response(ref response) = message {
						let id = response.id.clone();

						if let Some(active_request) = self.take_awaiting_request(&response.id).await {
							if let Err(_) = active_request.set_value(Arc::new(message)) {
								panic!("received Response (with request id {}) but failed to resolve the Request", id);
							}

							continue;
						}

						error!("Received Response (with request id {}) that had no associated Request.", id);

						continue;
					}

					panic!("expected message to be a 'Response' variant");
				},
				Err(err) => {
					error!("Unexpected error receiving response: {:?}", err);
				}
			}
		}

		if cancel_token.is_canceled() {
			return Err(OperationCanceled);
		}

		Ok(())
	}

	/// Sends a typed request to the LSP client and returns a `Future` that will yield its response.
	pub async fn send<T>(&self, params: T::Params) -> Result<T::Result, LspProtocolError>
	where
		T: analyzer_abstractions::lsp_types::request::Request + 'static,
		T::Params: Clone + Serialize + Send + Debug,
		T::Result: Clone + DeserializeOwned + Send + From<()>,
	{
		let id = RequestId::from(self.request_id.fetch_add(1, Ordering::Relaxed));
		let request = Request::new(id.clone(), T::METHOD.to_string(), params);
		let awaiting_request = self.create_active_request(&id).await;

		if let Err(_) = self.requests.send(Message::Request(request)).await {
			self.take_awaiting_request(&id).await; // Take the awaiting_request if we couldn't send the request.

			return Err(LspProtocolError::TransportError);
		}

		let response_message = awaiting_request.future().await?; // Wait for the request to complete.

		match &*response_message {
			Message::Response(response) => {
				if let Some(err) = &response.error {
					error!(method = T::METHOD, "Error processing response for server request '{}': {}", T::METHOD, err.message);

					return Err(LspProtocolError::UnexpectedResponse);
				}

				match response.result.as_ref() {
					Some(value) => Ok(from_json::<T::Result>(value)?),
					None => Ok(<T::Result as From<()>>::from(()))
				}
			},
			_ => Err(LspProtocolError::UnexpectedResponse)
		}
	}

	async fn create_active_request(&self, id: &RequestId) -> Arc<AnyFutureCompletionSource> {
		let mut awaiting_requests = self.awaiting_requests.write().await;

		awaiting_requests.insert(id.clone(), Arc::new(FutureCompletionSource::<Arc<Message>, LspProtocolError>::new()));

		awaiting_requests.get(&id).unwrap().clone()
	}

	async fn take_awaiting_request(&self, id: &RequestId) -> Option<Arc<AnyFutureCompletionSource>> {
		let mut awating_requests = self.awaiting_requests.write().await;

		awating_requests.remove(&id)
	}
}

impl Clone for RequestManager {
	/// Returns a copy of the [`RequestManager`].
	fn clone(&self) -> Self {
		Self {
			requests: self.requests.clone(),
			responses: self.responses.clone(),
			request_id: AtomicI32::new(self.request_id.load(Ordering::Relaxed)),
			awaiting_requests: self.awaiting_requests.clone()
		}
	}
}
