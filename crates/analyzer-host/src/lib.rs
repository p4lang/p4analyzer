mod fsm;
mod lsp;
mod lsp_impl;
pub mod json_rpc;
pub mod tracing;

use std::sync::Arc;
use analyzer_abstractions::{tracing::*};
use async_channel::{Receiver, Sender};
use cancellation::{CancellationToken, OperationCanceled};
use fsm::LspProtocolMachine;
use json_rpc::message::Message;
use tracing::TraceValueAccessor;

/// A tuple type that represents both a sender and a receiver of [`Message`] instances.
pub type MessageChannel = (Sender<Message>, Receiver<Message>);

/// Provides a runtime environment for the P4 Analyzer, utilizing services that are provided by the host process.
pub struct AnalyzerHost {
	sender: Sender<Message>,
	receiver: Receiver<Message>,
	trace_value: Option<TraceValueAccessor>
}

impl AnalyzerHost {
	/// Initializes a new [`AnalyzerHost`] instance with a [`MessageChannel`] to send and receive Language Server Protocol (LSP)
	/// messages over, and an optional [`TraceValueAccessor`] that can be used to set the LSP tracing value.
	///
	/// If available, `trace_value` will be used on receipt of a `'$/setTrace'` notification from the LSP client to set
	/// the required logging level.
	pub fn new(request_channel: MessageChannel, trace_value: Option<TraceValueAccessor>) -> Self {
		let (sender, receiver) = request_channel;

		AnalyzerHost {
			sender,
			receiver,
			trace_value
		}
	}

	/// Starts executing the the [`AnalyzerHost`] instance.
	///
	/// Once started, request messages will be received through the message channel, forwarded for processing to the internal
	/// state machine, with response messages sent back through the message channel for the client to process.
	pub async fn start(&self, cancel_token: Arc<CancellationToken>) -> Result<(), OperationCanceled> {
		info!("AnalyzerHost is starting.");

		let mut protocol_machine = LspProtocolMachine::new(self.trace_value.clone());

		while protocol_machine.is_active() && !cancel_token.is_canceled() {
			let request_message = self.receiver.recv().await;

			if cancel_token.is_canceled() {
				break;
			}

			match request_message {
				Ok(message) => {
					let request_message_span =info_span!("[Message]", message = format!("{}", message));

					async {
						match protocol_machine.process_message(&message).await {
							Ok(response_message) => {
								if let Some(Message::Response(_)) = &response_message {
									self.sender.send(response_message.unwrap()).await.unwrap();
								}
							}
							Err(err) => {
								error!("Protocol Error: {}", &err.to_string());
							}
						}
					}
					.instrument(request_message_span)
					.await;
				}
				Err(err) => {
					error!("Unexpected error receving request: {:?}", err);

					continue
				},
			}
		}

		info!("AnalyzerHost is stopping.");

		if protocol_machine.is_active() {
			return Err(OperationCanceled);
		}

		Ok(())
	}
}
