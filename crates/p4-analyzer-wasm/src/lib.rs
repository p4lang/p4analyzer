mod buffer;

extern crate console_error_panic_hook;

use analyzer_abstractions::{tracing::subscriber};
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
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct LspServer {
	on_response_callback: js_sys::Function,
	request_channel: MessageChannel,
	response_channel: MessageChannel,
	cts: CancellationTokenSource,
}

#[wasm_bindgen]
impl LspServer {
	/// Initializes a new [`LspServer`] instance.
	#[wasm_bindgen(constructor)]
	pub fn new(on_response_callback: js_sys::Function) -> Self {
		console_error_panic_hook::set_once();

		Self {
			on_response_callback,
			request_channel: async_channel::unbounded::<Message>(),
			response_channel: async_channel::unbounded::<Message>(),
			cts: CancellationTokenSource::new(),
		}
	}

	/// Starts the LSP server by creating and starting an underlying [`AnalyzerHost`].
	pub async fn start(&self) -> Result<JsValue, JsValue> {
		let layer = LspTracingLayer::new(self.get_message_channel());
		let host = AnalyzerHost::new(self.get_message_channel(), Some(layer.trace_value()), None);
		let subscriber = Registry::default().with(layer);

		subscriber::set_global_default(subscriber)
			.expect("Unable to set global tracing subscriber.");

		match join(host.start(self.cts.token().clone()), self.on_receive()).await {
			(Ok(_), Ok(_)) => Ok(JsValue::UNDEFINED),
			_ => Err(JsValue::from(Error::new("The server was stopped."))),
		}
	}

	/// Reads a request message from a given `Buffer`, and sends it to the underlying request channel for processing
	/// by the [`AnalyzerHost`].
	#[wasm_bindgen(js_name = "sendRequest")]
	pub async fn send_request(&self, request_buffer: Buffer) -> Result<JsValue, JsValue> {
		match serde_json::from_slice::<Message>(to_u8_vec(&request_buffer).as_slice()) {
			Ok(message) => {
				let (sender, _) = self.request_channel.clone();

				if sender.send(message).await.is_err() {
					return Err(JsValue::from(Error::new(
						"Unexpected error writing request message to request channel.",
					)));
				}

				Ok(JsValue::UNDEFINED)
			}
			Err(err) => Err(JsValue::from(Error::new(&format!(
				"Unexpected error reading request message: {}",
				err.to_string()
			)))),
		}
	}

	/// Stops the LSP server by cancelling all of its underlying operations.
	pub fn stop(&self) {
		self.cts.cancel();

		let (sender, receiver) = self.get_message_channel();

		sender.close();
		receiver.close();
	}

	/// Asynchronously receives response messages from the response channel, converts them to a `Buffer`, and
	/// then invokes the receiver callback provided by the JavaScript host.
	async fn on_receive(&self) -> Result<(), OperationCanceled> {
		#[derive(Serialize)]
		struct JsonRpcEnvelope {
			jsonrpc: &'static str,

			#[serde(flatten)]
			msg: Message,
		}

		let (_, receiver) = self.response_channel.clone();

		while let Ok(message) = receiver.recv().await {
			let message_text = serde_json::to_string(&JsonRpcEnvelope {
				jsonrpc: "2.0",
				msg: message,
			})
			.unwrap();

			self.on_response_callback
				.call1(&JsValue::NULL, &to_buffer(message_text.as_bytes()))
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

	#[wasm_bindgen(js_namespace = console)]
	fn error(s: &str);
}
