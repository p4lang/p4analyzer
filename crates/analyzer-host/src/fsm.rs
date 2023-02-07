use crate::json_rpc::{from_json, message::*, DeserializeError, ErrorCode};
use crate::tracing::TraceValueAccessor;
use analyzer_abstractions::lsp_types::{
	CompletionOptions, DeclarationCapability, DidChangeTextDocumentParams,
	DidCloseTextDocumentParams, DidOpenTextDocumentParams, HoverProviderCapability,
	ImplementationProviderCapability, InitializeResult, OneOf, ServerCapabilities, ServerInfo,
	SignatureHelpOptions, TextDocumentSyncCapability, TextDocumentSyncKind, TraceValue,
	TypeDefinitionProviderCapability, WorkDoneProgressOptions, CompletionParams, CompletionResponse, CompletionList, CompletionItem, HoverParams, Hover, HoverContents, MarkupContent, MarkupKind, CompletionItemKind, SetTraceParams,
};
use analyzer_abstractions::tracing::{error, info};
use analyzer_abstractions::{lsp_types::InitializeParams};
use thiserror::Error;

/// Represents the valid states of a [`ProtocolMachine`].
#[derive(Debug, PartialEq, PartialOrd, Copy, Clone)]
pub(crate) enum ProtocolState {
	/// The machine is active, but has not yet received an initialization request from the client.
	ActiveUninitialized,

	/// The machine is currently processing an initialization request from the client.
	Initializing,

	/// The machine is active and ready to process requests from the client.
	ActiveInitialized,

	/// The machine is currently processing a shutdown request from the client.
	ShuttingDown,

	/// The machine has shutdown and will no longer process requests from the client.
	Stopped,
}

/// Represents an error in protocol while processing a received client message.
#[derive(Error, Debug)]
pub enum ProtocolError {
	/// The received request was not expected.
	#[error("The received request was not expected.")]
	UnexpectedRequest,

	/// The received request was was malformed or invalid
	#[error("The received request was was malformed or invalid.")]
	BadRequest(#[from] DeserializeError),
}

/// A state machine that models the Language Server Protocol (LSP). In the specification, a LSP server has a lifecycle
/// that is managed fully by the client. [`ProtocolMachine`] ensures that the server responds accordingly by
/// transitioning itself through states based on the requests received, and then processed on behalf of the client. If
/// the server is in an invalid state for a given request, then the client will receive an appropriate error response.
#[derive(Clone)]
pub(crate) struct ProtocolMachine {
	/// The current [`ProtocolState`].
	pub(crate) current_state: ProtocolState,
	trace_value: Option<TraceValueAccessor>,
}

impl ProtocolMachine {
	/// Initializes a new [`ProtocolMachine`] that will start in the [`ProtocolState::ActiveUninitialized`] state.
	pub fn new(trace_value: Option<TraceValueAccessor>) -> Self {
		ProtocolMachine {
			current_state: ProtocolState::ActiveUninitialized,
			trace_value
		}
	}

	/// Returns `true` if the current [`ProtocolMachine`] is in an active state; otherwise `false`.
	pub fn is_active(&self) -> bool {
		self.current_state != ProtocolState::Stopped
	}

	/// Processes a [`Message`] for the current [`ProtocolState`], and returns an optional [`Message`] that represents its response.
	///
	/// If the supplied message yields a 'bad request' response (i.e., it contains malformed or invalid parameter data), then the
	/// [`ProtocolMachine`] will transition back to the state it was in prior to processing the supplied message (but only if the
	/// state has not transitioned to [`ProtocolState::ActiveInitialized`]).
	pub async fn process_message(
		&mut self,
		message: Message,
	) -> Result<Option<Message>, ProtocolError> {
		let current_state = self.current_state;
		let result = match self.current_state {
			ProtocolState::ActiveUninitialized => self.on_active_uninitialized(message).await,
			ProtocolState::Initializing => self.on_initializing(message).await,
			ProtocolState::ActiveInitialized => self.on_active_initialized(message).await,
			ProtocolState::ShuttingDown => self.on_shutting_down(message).await,
			// We should receive no messages when in the `Stopped` state.
			ProtocolState::Stopped => Err(ProtocolError::UnexpectedRequest),
		};

		// If we encounter any ProtocolError, then move back to the previous state if we've not yet been initialized.
		if result.is_err() && current_state < ProtocolState::ActiveInitialized {
			self.transition_to(current_state);
		}

		result
	}

	/// Message handling for the [`ProtocolState::Initializing`] state.
	async fn on_initializing(
		&mut self,
		message: Message,
	) -> Result<Option<Message>, ProtocolError> {
		match message {
			Message::Notification(notification) if notification.is_exit() => {
				info!("Received 'exit' notification. Server is now stopping.");

				self.transition_to(ProtocolState::Stopped);
				Ok(None)
			}

			Message::Notification(notification) if notification.is_initialized() => {
				info!("Received 'initialized' notification. Server is now ready for document synchronization.");

				self.transition_to(ProtocolState::ActiveInitialized);
				Ok(None)
			}

			// Reject any other request.
			Message::Request(request) => {
				error!(
					"Received '{}' request. Responding with 'ServerNotInitialized' error code.",
					request.method
				);

				let response = Response::new_error(
					request.id,
					ErrorCode::ServerNotInitialized as i32,
					"The server is currently initializing.",
				);

				Ok(Some(Message::Response(response)))
			}

			// Ignore any other notifications.
			Message::Notification(notification) => {
				info!("Received '{}' notification. Ignoring.", notification.method);

				Ok(None)
			}

			_ => Err(ProtocolError::UnexpectedRequest),
		}
	}

	/// Message handling for the [`ProtocolState::ActiveUninitialized`] state.
	async fn on_active_uninitialized(
		&mut self,
		message: Message,
	) -> Result<Option<Message>, ProtocolError> {
		match message {
			// Process an 'exit' notification by immediately transitioning to 'stopped'.
			Message::Notification(notification) if notification.is_exit() => {
				info!("Received 'exit' notification. Server is now stopping.");

				self.transition_to(ProtocolState::Stopped);
				Ok(None)
			}

			// Process an 'initialize' request.
			Message::Request(request) if request.is_initialize() => {
				info!("Received 'initialize' request. Server is now initializing.");

				self.transition_to(ProtocolState::Initializing);

				let params = from_json::<InitializeParams>("InitializeParams", &request.params)?;
				let trace = params.trace.unwrap_or(TraceValue::Off);

				info!("Tracevalue = {:?}", trace);

				let result = InitializeResult {
					capabilities: ServerCapabilities {
						text_document_sync: Some(TextDocumentSyncCapability::Kind(
							TextDocumentSyncKind::INCREMENTAL,
						)),
						completion_provider: Some(CompletionOptions {
							resolve_provider: Some(true),
							trigger_characters: Some(vec![
								"(".to_string(),
								"<".to_string(),
								".".to_string(),
							]),
							all_commit_characters: None,
							work_done_progress_options: WorkDoneProgressOptions {
								work_done_progress: None,
							},
						}),
						hover_provider: Some(HoverProviderCapability::Simple(true)),
						signature_help_provider: Some(SignatureHelpOptions {
							trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
							retrigger_characters: None,
							work_done_progress_options: WorkDoneProgressOptions {
								work_done_progress: None,
							},
						}),
						declaration_provider: Some(DeclarationCapability::Simple(true)),
						definition_provider: Some(OneOf::Left(true)),
						type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(
							true,
						)),
						implementation_provider: Some(ImplementationProviderCapability::Simple(
							true,
						)),
						references_provider: Some(OneOf::Left(true)),
						..Default::default()
					},
					server_info: Some(ServerInfo {
						name: String::from("p4-analyzer"),
						version: Some(String::from("0.0.0")),
					}),
				};

				info!("TODO: Process and initialize workspace roots.");

				Ok(Some(Message::Response(Response::new(request.id, result))))
			}

			// Reject any other request.
			Message::Request(request) => {
				error!(
					"Received '{}' request. Responding with 'ServerNotInitialized' error code.",
					request.method
				);

				let response = Response::new_error(
					request.id,
					ErrorCode::ServerNotInitialized as i32,
					"An 'initialize' request is required.",
				);

				Ok(Some(Message::Response(response)))
			}

			// Ignore any other notifications.
			Message::Notification(notification) => {
				info!("Received '{}' notification. Ignoring.", notification.method);

				Ok(None)
			}

			_ => Err(ProtocolError::UnexpectedRequest),
		}
	}

	/// Message handling for the [`ProtocolState::ActiveInitialized`] state.
	async fn on_active_initialized(
		&mut self,
		message: Message,
	) -> Result<Option<Message>, ProtocolError> {
		match message {
			Message::Notification(notification) if notification.is_exit() => {
				info!("Received 'exit' notification. Server is now stopping.");

				self.transition_to(ProtocolState::Stopped);
				Ok(None)
			}

			Message::Request(request) if request.is_shutdown() => {
				info!("Received 'shutdown' request. Server is now shutting down.");

				self.transition_to(ProtocolState::ShuttingDown);

				info!("TODO: Shutdown the server.");

				Ok(Some(Message::Response(Response::new(
					request.id,
					serde_json::Value::Null,
				))))
			}

			Message::Notification(notification) if notification.is("$/setTrace") => {
				if let Some(trace_value) = &self.trace_value {
					let params = from_json::<SetTraceParams>("SetTraceParams", &notification.params)?;

					trace_value.set(params.value);
				}

				Ok(None)
			}

			Message::Notification(notification) if notification.is("textDocument/didOpen") => {
				info!("Received 'textDocument/didOpen' notification.");

				let params = from_json::<DidOpenTextDocumentParams>(
					"DidOpenTextDocumentParams",
					&notification.params,
				)?;

				info!(
					"{} (version = {})",
					params.text_document.uri, params.text_document.version
				);

				Ok(None)
			}

			Message::Notification(notification) if notification.is("textDocument/didChange") => {
				info!("Received 'textDocument/didChange' notification.");

				let params = from_json::<DidChangeTextDocumentParams>(
					"DidChangeTextDocumentParams",
					&notification.params,
				)?;

				info!(
					"{} (version = {}, content_changes = {})",
					params.text_document.uri,
					params.text_document.version,
					params.content_changes.len()
				);

				Ok(None)
			}

			Message::Notification(notification) if notification.is("textDocument/didClose") => {
				info!("Received 'textDocument/didClose' notification.");

				let params = from_json::<DidCloseTextDocumentParams>(
					"DidCloseTextDocumentParams",
					&notification.params,
				)?;

				info!("{}", params.text_document.uri);

				Ok(None)
			}

			Message::Request(request) if request.is("textDocument/hover") => {
				info!("Received 'textDocument/completion' request.");

				let params = from_json::<HoverParams>("HoverParams", &request.params)?;

				let line = params.text_document_position_params.position.line;
				let character = params.text_document_position_params.position.character;

				let hover = Hover {
					range: None,
					contents: HoverContents::Markup(MarkupContent{
						kind: MarkupKind::Markdown,
						value: format!("Hovering over Ln *{}*, Col *{}*.", line, character)
					})
				};

				Ok(Some(Message::Response(Response::new(
					request.id,
					hover,
				))))
			}

			Message::Request(request) if request.is("textDocument/completion") => {
				info!("Received 'textDocument/completion' request.");

				let params = from_json::<CompletionParams>("dsd", &request.params)?;

				let line = params.text_document_position.position.line;

				let data = CompletionList {
					is_incomplete: false,
					items: vec![
						CompletionItem {
							label: String::from("p4"),
							kind: Some(CompletionItemKind::FILE),
							..Default::default()
						},
						CompletionItem {
							label: String::from("rules"),
							kind: Some(CompletionItemKind::FILE),
							..Default::default()
						}
					]
				};

				Ok(Some(Message::Response(Response::new(
					request.id,
					data,
				))))
			}

			Message::Notification(notification) => {
				info!("Received '{}' notification.", notification.method);

				Ok(None)
			}

			Message::Request(request) => {
				info!("Received '{}' request.", request.method);

				Ok(None)
			}

			_ => Err(ProtocolError::UnexpectedRequest),
		}
	}

	async fn on_shutting_down(
		&mut self,
		message: Message,
	) -> Result<Option<Message>, ProtocolError> {
		match message {
			Message::Notification(notification) if notification.is_exit() => {
				info!("Received 'exit' notification. Server is now stopping.");

				self.transition_to(ProtocolState::Stopped);
				Ok(None)
			}

			Message::Request(request) => {
				error!(
					"Received '{}' request. Responding with 'InvalidRequest' error code.",
					request.method
				);

				let response = Response::new_error(
					request.id,
					ErrorCode::InvalidRequest as i32,
					"The server is currently shutting down.",
				);

				Ok(Some(Message::Response(response)))
			}

			// Ignore any other notifications.
			Message::Notification(notification) => {
				info!("Received '{}' notification. Ignoring.", notification.method);

				Ok(None)
			}

			_ => Err(ProtocolError::UnexpectedRequest),
		}
	}

	/// Transitions the current [`ProtocolMachine`] to a new state.
	///
	/// Once transitioned, the machine will begin processing requests from the client appropriately to that state.
	fn transition_to(&mut self, target_state: ProtocolState) {
		self.current_state = target_state;
	}
}
