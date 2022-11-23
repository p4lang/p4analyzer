mod buffer;

use analyzer_abstractions::{tracing::subscriber, Logger, lsp_types::request};
use analyzer_host::{
	json_rpc::message::*,
	tracing::{
		tracing_subscriber::{prelude::*, Registry},
		LspTracingLayer,
	},
	AnalyzerHost, MessageChannel,
};
use buffer::{Buffer, to_buffer, to_u8_vec};
use cancellation::{CancellationToken, CancellationTokenSource, OperationCanceled};
use futures::future::{join, join_all};
use js_sys::{Error, Promise};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct LspServer {
	input_buffer: Vec<u8>,
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
		Self {
			input_buffer: Vec::<u8>::new(),
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
			(Ok(_), Ok(_)) => Ok(JsValue::undefined()),
			_ => Err(JsValue::from(Error::new("The server was stopped."))),
		}
	}

	/// Updates the underlying input buffer and attempts to read a [`Message`] from it. If a [`Message`] could be read
	/// from the input buffer then it is sent to the underlying request channel for processing by the [`AnalyzerHost`].
	#[wasm_bindgen(js_name = "sendRequestBuffer")]
	pub async fn send_request_buffer(&mut self, request_buffer: Buffer) -> Result<JsValue, JsValue> {
		self.input_buffer.append(&mut to_u8_vec(&request_buffer.buffer()));

		let mut input_buffer_slice = &self.input_buffer[..];

		if let Ok(Some(message)) = Message::read(&mut input_buffer_slice) {
			let (sender, _) = self.request_channel.clone();

			if sender.send(message).await.is_err() {
				return Err(JsValue::from(Error::new("Unexpected error writing request message to request channel.")));
			}
		}

		Ok(JsValue::undefined())
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
		let this = JsValue::null();

		while let Ok(message) = receiver.recv().await {
			let mut buffer = Vec::<u8>::new();

			message.write(&mut buffer).unwrap();
			self.on_receive_cb.call1(&this, &to_buffer(&buffer)).unwrap();
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
