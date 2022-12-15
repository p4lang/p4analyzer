pub use tracing_subscriber;

use crate::{
	json_rpc::message::{Message, Notification},
	MessageChannel,
};
use analyzer_abstractions::{
	lsp_types::{LogMessageParams, LogTraceParams, MessageType},
	tracing::field::Field,
};
use async_channel::Sender;
use std::fmt::{Display, Write};
use tracing_subscriber::{field::Visit, Layer};

struct LspTracingMessageVisitor {
	message: String,
	formatted_fields: String,
}

impl LspTracingMessageVisitor {
	fn new() -> Self {
		Self {
			message: String::new(),
			formatted_fields: String::new(),
		}
	}
}

impl Display for LspTracingMessageVisitor {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		if !self.formatted_fields.is_empty() {
			write!(f, "{} [{}]", self.message, self.formatted_fields)?;
			return Ok(());
		}

		write!(f, "{}", self.message)?;
		Ok(())
	}
}

impl Visit for LspTracingMessageVisitor {
	fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
		if field.name() == "message" {
			write!(self.message, "{:?}", value).unwrap();

			return;
		}

		// Ignore `lsp_event` fields specifically, as these will always be present.
		if field.name() != "lsp_event" {
			write!(self.formatted_fields, "{}={:?},", field.name(), value).unwrap();
		}
	}
}

pub struct LspTracingLayer {
	sender: Sender<Message>,
}

impl LspTracingLayer {
	pub fn new(request_channel: MessageChannel) -> Self {
		let (sender, _) = request_channel;

		Self { sender }
	}
}

impl<S> Layer<S> for LspTracingLayer
where
	S: analyzer_abstractions::tracing::Subscriber,
{
	fn on_event(
		&self,
		event: &analyzer_abstractions::tracing::Event,
		_ctx: tracing_subscriber::layer::Context<S>,
	) {
		// Ignore any events that do not carry a `lsp_event` field.
		// if !event.fields().any(|field| field.name() == "lsp_event") {
		// 	return;
		// }

		let mut visitor = LspTracingMessageVisitor::new();

		event.record(&mut visitor);

		// let notification = Notification::new(
		// 	"$/logTrace",
		// 	LogTraceParams {
		// 		message: visitor.message,
		// 		verbose: None,
		// 	},
		// );

		let notification = Notification::new(
			"window/logMessage",
			LogMessageParams {
				message: visitor.message,
				typ: MessageType::LOG,
			},
		);

		self.sender
			.send_blocking(Message::Notification(notification))
			.unwrap_or_default(); // Ignore errors.
	}
}
