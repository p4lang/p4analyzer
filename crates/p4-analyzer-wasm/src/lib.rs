mod buffer;

extern crate console_error_panic_hook;

use analyzer_abstractions::{tracing::subscriber, Logger};
use analyzer_host::{
	json_rpc::message::*,
	tracing::{
		tracing_subscriber::{prelude::*, Registry},
		LspTracingLayer,
	},
	AnalyzerHost, MessageChannel,
};
use buffer::{to_buffer, to_u8_vec, Buffer};
use cancellation::{CancellationTokenSource, OperationCanceled};
use futures::future::join;
use js_sys::Error;
use std::panic;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct LspServer {
	on_receive_cb: js_sys::Function,
	request_channel: MessageChannel,
	response_channel: MessageChannel,
	cts: CancellationTokenSource,
}

#[wasm_bindgen]
impl LspServer {
	/// Initializes a new [`LspServer`] instance.
	#[wasm_bindgen(constructor)]
	pub fn new(on_receive_cb: js_sys::Function) -> Self {
		console_error_panic_hook::set_once();

		Self {
			on_receive_cb,
			request_channel: async_channel::unbounded::<Message>(),
			response_channel: async_channel::unbounded::<Message>(),
			cts: CancellationTokenSource::new(),
		}
	}

	/// Starts the LSP server by creating and starting an underlying [`AnalyzerHost`].
	pub async fn start(&self) -> Result<JsValue, JsValue> {
		let host = AnalyzerHost::new(self.get_message_channel(), &ConsoleLogger {});
		let subscriber = Registry::default().with(LspTracingLayer::new(self.get_message_channel()));

		subscriber::set_global_default(subscriber)
			.expect("Unable to set global tracing subscriber.");

		match join(host.start(&self.cts), self.on_receive()).await {
			(Ok(_), Ok(_)) => Ok(JsValue::UNDEFINED),
			_ => Err(JsValue::from(Error::new("The server was stopped."))),
		}
	}

	/// Updates the underlying input buffer and attempts to read a [`Message`] from it. If a [`Message`] could be read
	/// from the input buffer then it is sent to the underlying request channel for processing by the [`AnalyzerHost`].
	#[wasm_bindgen(js_name = "sendRequestBuffer")]
	pub async fn send_request_buffer(&self, request_buffer: Buffer) -> Result<Buffer, JsValue> {
		let mut message_buffer = &to_u8_vec(&request_buffer.buffer())[..];

		match Message::read(&mut message_buffer) {
			Ok(Some(message)) => {
				let (sender, _) = self.request_channel.clone();

				if sender.send(message).await.is_err() {
					return Err(JsValue::from(Error::new("Unexpected error writing request message to request channel.")));
				}

				Ok(to_buffer(message_buffer)) // Return a buffer over the modified `message_buffer`.
			}
			_ => Ok(request_buffer), // Return the unmodified `request_buffer`.
		}
	}

	/// Stops the LSP server by cancelling all of its underlying operations.
	pub fn stop(&self) {
		self.cts.cancel();

		let (sender, receiver) = self.get_message_channel();

		sender.close();
		receiver.close();
	}

	/// Asynchronously receives response messages from the response channel, converts them to a Buffer instance, and
	/// then invokes the receiver callback provided by the JavaScript host.
	async fn on_receive(&self) -> Result<(), OperationCanceled> {
		let (_, receiver) = self.response_channel.clone();

		while let Ok(message) = receiver.recv().await {
			let mut buffer = Vec::<u8>::new();

			message.write(&mut buffer).unwrap();
			self.on_receive_cb
				.call1(&JsValue::NULL, &to_buffer(&buffer))
				.unwrap();
		}

		Ok(())
	}

	fn get_message_channel(&self) -> MessageChannel {
		let (sender, _) = self.response_channel.clone();
		let (_, receiver) = self.request_channel.clone();

		(sender, receiver)
	}
}

#[wasm_bindgen]
extern "C" {
	#[wasm_bindgen(js_namespace = console)]
	fn log(s: &str);
}

struct ConsoleLogger {}

impl Logger for ConsoleLogger {
	fn log_message(&self, msg: &str) {
		log(msg);
	}

	fn log_error(&self, msg: &str) {
		log(msg);
	}
}
