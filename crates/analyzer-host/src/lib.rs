mod fsm;
pub mod json_rpc;
pub mod tracing;

use analyzer_abstractions::{tracing::*, LoggerImpl};
use async_channel::{Receiver, Sender};
use cancellation::{CancellationToken, OperationCanceled};
use fsm::ProtocolMachine;
use json_rpc::message::Message;

/// A tuple type that represents both a sender and a receiver of [`Message`] instances.
pub type MessageChannel = (Sender<Message>, Receiver<Message>);

/// Provides a runtime environment for the P4 Analyzer, utilizing services that are provided by the host process.
pub struct AnalyzerHost<'host> {
	sender: Sender<Message>,
	receiver: Receiver<Message>,

	/// A logger that the `AnalyzerHost` will use to output log messages.
	logger: &'host LoggerImpl,
}

impl<'host> AnalyzerHost<'host> {
	/// Initializes a new [`AnalyzerHost`] instance with a [`MessageChannel`] to send and receive Language Server Protocol (LSP)
	/// messages over, and a specified logger.
	pub fn new(request_channel: MessageChannel, logger: &'host LoggerImpl) -> Self {
		let (sender, receiver) = request_channel;

		AnalyzerHost {
			sender,
			receiver,
			logger,
		}
	}

	/// Starts executing the the [`AnalyzerHost`] instance.
	///
	/// Once started, request messages will be received through the message channel, forwarded for processing to the internal
	/// state machine, with response messages sent back through the message channel for the client to process.
	pub async fn start(&self, cancel_token: &CancellationToken) -> Result<(), OperationCanceled> {
		info!(lsp_event = true, "AnalyzerHost is starting.");

		let mut protocol_machine = ProtocolMachine::new(self.logger);

		while protocol_machine.is_active() && !cancel_token.is_canceled() {
			let request_message = self.receiver.recv().await;

			if cancel_token.is_canceled() {
				break;
			}

			match request_message {
				Ok(message) => {
					let request_message_span =
						info_span!("[Request Message]", message = format!("{}", message));

					async {
						let response_message = protocol_machine.process_message(message).await;

						match response_message {
							Ok(message) => {
								if let Some(Message::Response(_)) = &message {
									self.sender.send(message.unwrap()).await.unwrap();
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

		info!(lsp_event = true, "AnalyzerHost is stopping.");

		if protocol_machine.is_active() {
			return Err(OperationCanceled);
		}

		Ok(())
	}
}
