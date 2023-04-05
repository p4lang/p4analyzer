pub use tracing_subscriber;

use crate::{
	json_rpc::message::{Message, Notification},
	MessageChannel
};
use analyzer_abstractions::{
	lsp_types::{LogTraceParams, TraceValue},
	tracing::{field::Field, Event, Subscriber}
};
use async_channel::Sender;
use core::fmt::Debug;
use std::{
	fmt::{Display, Write},
	sync::{Arc, Mutex}
};
use tracing_subscriber::{field::Visit, layer::Context, Layer};

/// Allows the [`TraceValue`] to be set for a [`LspTracingLayer`].
///
/// Since `'Tracing'` logging levels have to be owned by subscribers, a [`TraceValueAccessor`] can be retrieved and then
/// later used to change the [`TraceValue`] used in order to determine how the received trace events should be processed
/// on behalf of the Language Server Protocol ('LSP') client.
#[derive(Clone)]

pub struct TraceValueAccessor(Arc<Mutex<TraceValue>>);

impl TraceValueAccessor {
	/// Sets the [`TraceValue`] on the associated [`LspTracingLayer`].

	pub fn set(&self, new_trace_value: TraceValue) {

		let TraceValueAccessor(trace_value) = self;

		let mut trace_value = trace_value.lock().unwrap();

		*trace_value = new_trace_value;
	}
}

/// A `'Tracing'` logging layer that writes messages to a message channel attached to a Language Server Protocol ('LSP') client.

pub struct LspTracingLayer {
	sender: Sender<Message>,
	trace_value: Arc<Mutex<TraceValue>>
}

impl LspTracingLayer {
	/// Initializes a new [`LspTracingLayer`] that will write trace messages to a given [`MessageChannel`].

	pub fn new(request_channel: MessageChannel) -> Self {

		let (sender, _) = request_channel;

		Self { sender, trace_value: Arc::new(Mutex::new(TraceValue::Off)) }
	}

	pub fn trace_value(&self) -> TraceValueAccessor { TraceValueAccessor(self.trace_value.clone()) }
}

impl<S> Layer<S> for LspTracingLayer
where
	S: Subscriber
{
	fn on_event(&self, event: &Event, _ctx: Context<S>) {

		let trace_value = self.trace_value.lock().unwrap();

		if *trace_value == TraceValue::Off {

			return;
		}

		let mut visitor = LspTraceMessageVisitor::new();

		event.record(&mut visitor);

		let notification = Notification::new(
			"$/logTrace",
			LogTraceParams {
				message: visitor.message,
				verbose: if *trace_value == TraceValue::Verbose { Some(visitor.formatted_fields) } else { None }
			}
		);

		self.sender.send_blocking(Message::Notification(notification)).unwrap_or_default();
		// Ignore errors.
	}
}

/// Provides a [`Visit`] implementation that allows trace events to be formatted for a message channel attached to a Language
/// Server Client.

struct LspTraceMessageVisitor {
	message: String,
	formatted_fields: String
}

impl LspTraceMessageVisitor {
	fn new() -> Self { Self { message: String::new(), formatted_fields: String::new() } }
}

impl Display for LspTraceMessageVisitor {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {

		if !self.formatted_fields.is_empty() {

			write!(f, "{} [{}]", self.message, self.formatted_fields)?;

			return Ok(());
		}

		write!(f, "{}", self.message)?;

		Ok(())
	}
}

impl Visit for LspTraceMessageVisitor {
	fn record_debug(&mut self, field: &Field, value: &dyn Debug) {

		if field.name() == "message" {

			write!(self.message, "{:?}", value).unwrap();

			return;
		}

		write!(self.formatted_fields, "{}={:?},", field.name(), value).unwrap();
	}
}
