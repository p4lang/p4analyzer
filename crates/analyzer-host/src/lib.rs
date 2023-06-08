pub mod fs;
mod fsm;
pub mod json_rpc;
pub mod lsp;
mod lsp_impl;
pub mod tracing;

use analyzer_abstractions::{
	fs::AnyEnumerableFileSystem, futures::future::join4 as join_all, futures_extensions::async_extensions::AsyncPool,
	tracing::*,
};
use async_channel::{Receiver, Sender};
use cancellation::{CancellationToken, OperationCanceled};
use fs::LspEnumerableFileSystem;
use fsm::LspProtocolMachine;
use json_rpc::message::Message;
use std::sync::Arc;
use tracing::TraceValueAccessor;

use crate::lsp::request::RequestManager;

/// A tuple type that represents both a sender and a receiver of [`Message`] instances.
pub type MessageChannel = (Sender<Message>, Receiver<Message>);

/// Provides a runtime environment for the P4 Analyzer, utilizing services that are provided by the host process.
pub struct AnalyzerHost {
	sender: Sender<Message>,
	receiver: Receiver<Message>,
	trace_value: Option<TraceValueAccessor>,
	file_system: Option<Arc<AnyEnumerableFileSystem>>,
}

impl AnalyzerHost {
	/// Initializes a new [`AnalyzerHost`] instance with a [`MessageChannel`] to send and receive Language Server Protocol (LSP)
	/// messages over, and an optional [`TraceValueAccessor`] that can be used to set the LSP tracing value.
	///
	/// If available, `trace_value` will be used on receipt of a `'$/setTrace'` notification from the LSP client to set
	/// the required logging level.
	pub fn new(
		message_channel: MessageChannel,
		trace_value: Option<TraceValueAccessor>,
		file_system: Option<Arc<AnyEnumerableFileSystem>>,
	) -> Self {
		let (sender, receiver) = message_channel;

		AnalyzerHost { sender, receiver, trace_value, file_system }
	}

	/// Starts executing the [`AnalyzerHost`] instance.
	///
	/// Once started, request messages will be received through the message channel, forwarded for processing to the internal
	/// state machine, with response messages sent back through the message channel for the client to process.
	pub async fn start(&self, cancel_token: Arc<CancellationToken>) -> Result<(), OperationCanceled> {
		let (requests_sender, requests_receiver) = async_channel::unbounded::<Message>();
		let (responses_sender, responses_receiver) = async_channel::unbounded::<Message>();
		let request_manager = RequestManager::new((self.sender.clone(), responses_receiver.clone()));
		// If no file system was supplied, then default to the standard LSP based one.
		let file_system: Arc<AnyEnumerableFileSystem> = match self.file_system.as_ref() {
			Some(fs) => fs.clone(),
			None => Arc::new(Box::new(LspEnumerableFileSystem::new(request_manager.clone()))),
		};

		match join_all(
			self.receive_messages(requests_sender.clone(), responses_sender.clone(), cancel_token.clone()),
			self.run_protocol_machine(
				request_manager.clone(),
				file_system,
				requests_receiver.clone(),
				cancel_token.clone(),
			),
			request_manager.start(cancel_token.clone()),
			AsyncPool::start(cancel_token.clone()),
		)
		.await
		{
			(Ok(_), Ok(_), Ok(_), Ok(_)) => {
				self.sender.close();
				self.receiver.close();

				Ok(())
			}
			_ => Err(OperationCanceled),
		}
	}

	async fn receive_messages(
		&self,
		requests_sender: Sender<Message>,
		responses_sender: Sender<Message>,
		cancel_token: Arc<CancellationToken>,
	) -> Result<(), OperationCanceled> {
		while !cancel_token.is_canceled() {
			match self.receiver.recv().await {
				Ok(message) => match message {
					Message::Response(_) => responses_sender.send(message).await.unwrap(),
					_ => requests_sender.send(message).await.unwrap(),
				},
				Err(err) => {
					error!("Unexpected error receiving message: {:?}", err);
					break;
				}
			}
		}

		requests_sender.close();
		responses_sender.close();

		return Err(OperationCanceled);
	}

	async fn run_protocol_machine(
		&self,
		request_manager: RequestManager,
		file_system: Arc<AnyEnumerableFileSystem>,
		requests_receiver: Receiver<Message>,
		cancel_token: Arc<CancellationToken>,
	) -> Result<(), OperationCanceled> {
		{
			// Scope: for `protocol_machine`.
			let mut protocol_machine = LspProtocolMachine::new(self.trace_value.clone(), request_manager, file_system);

			while protocol_machine.is_active() && !cancel_token.is_canceled() {
				match requests_receiver.recv().await {
					Ok(message) => {
						if cancel_token.is_canceled() {
							break;
						}

						let request_message_span = info_span!("[Message]", message = format!("{}", message));

						async {
							match protocol_machine.process_message(Arc::new(message)).await {
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
						error!("Unexpected error receiving request or notification: {:?}", err);
					}
				}
			}
		} // End Scope

		self.sender.close();
		self.receiver.close();
		requests_receiver.close();
		AsyncPool::stop();

		if cancel_token.is_canceled() {
			return Err(OperationCanceled);
		}

		Ok(())
	}
}
