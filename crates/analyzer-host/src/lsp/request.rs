use std::{collections::HashMap, sync::{Arc, atomic::{AtomicI32, Ordering}}};
use async_rwlock::RwLock as AsyncRwLock;
use analyzer_abstractions::{futures::FutureCompletionSource, tracing::error};
use async_channel::Sender;

use crate::json_rpc::{message::{Message, Request}, RequestId, from_json};
use serde::{de::DeserializeOwned, Serialize};

use super::LspProtocolError;

type AnyFutureCompletionSource = FutureCompletionSource<Arc<Message>, LspProtocolError>;

pub(crate) struct RequestManager {
	sender: Sender<Message>,
	request_id: AtomicI32,
	active_requests: Arc<AsyncRwLock<HashMap<RequestId, Arc<AnyFutureCompletionSource>>>>,
}

impl RequestManager {
	pub fn new(sender: Sender<Message>) -> Self {
		Self {
			sender,
			request_id: AtomicI32::new(0),
			active_requests: Arc::new(AsyncRwLock::new(HashMap::new())),
		}
	}

	pub async fn send<T>(&self, params: T::Params) -> Result<T::Result, LspProtocolError>
	where
		T: analyzer_abstractions::lsp_types::request::Request + 'static,
		T::Params: Clone + Serialize + Send + std::fmt::Debug,
		T::Result: Clone + DeserializeOwned + Send,
	{
		let id = RequestId::from(self.request_id.fetch_add(1, Ordering::Relaxed));
		let request = Request::new(id.clone(), T::METHOD.to_string(), params);
		let active_request = self.create_active_request(&id).await;

		if let Err(_) = self.sender.send(Message::Request(request)).await {
			self.take_active_request(&id).await; // Take the active_request if we couldn't send the request.

			return Err(LspProtocolError::TransportError);
		}

		let response_message = active_request.future().await?; // Wait for the active request to complete.

		match &*response_message {
			Message::Response(response) => {
				if let Some(err) = &response.error {
					error!(method = T::METHOD, "Error processing response for server request '{}': {}", T::METHOD, err.message);

					return Err(LspProtocolError::UnexpectedResponse);
				}

				Ok(from_json::<T::Result>(&response.result.as_ref().unwrap())?)
			},
			_ => Err(LspProtocolError::UnexpectedResponse)
		}
	}

	pub async fn process_response(&self, message: Arc<Message>) -> Result<(), LspProtocolError> {
		if let Message::Response(ref response) = *message {
			let id = response.id.clone();

			if let Some(active_request) = self.take_active_request(&response.id).await {
				return match active_request.set_value(message.clone()).await {
					Ok(r) => Ok(r),
					Err(_) => {
						error!("Received Response (with request id {}) but could not set it for the Request.", id);

						Err(LspProtocolError::UnexpectedResponse)
					}
				}
			}

			error!("Received Response (with request id {}) that had no matching request.", id);

			return Err(LspProtocolError::UnexpectedResponse);
		}

		panic!("expected message to be a 'Response' variant");
	}

	async fn create_active_request(&self, id: &RequestId) -> Arc<AnyFutureCompletionSource> {
		let mut active_requests = self.active_requests.write().await;

		active_requests.insert(id.clone(), Arc::new(FutureCompletionSource::<Arc<Message>, LspProtocolError>::new()));

		active_requests.get(&id).unwrap().clone()
	}

	async fn take_active_request(&self, id: &RequestId) -> Option<Arc<AnyFutureCompletionSource>> {
		let mut active_requests = self.active_requests.write().await;

		active_requests.remove(&id)
	}
}

impl Clone for RequestManager {
	fn clone(&self) -> Self {
		Self {
			sender: self.sender.clone(),
			request_id: AtomicI32::new(self.request_id.load(Ordering::Relaxed)),
			active_requests: self.active_requests.clone()
		}
	}
}
