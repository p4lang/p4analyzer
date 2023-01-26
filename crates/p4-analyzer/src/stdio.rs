use analyzer_host::{json_rpc::message::Message, MessageChannel};
use cancellation::{CancellationToken, OperationCanceled};
use tokio::task;
use std::{io::{stdin, stdout}, sync::Arc};

/// Connects the `stdin` and `stdout` of the process to appropriate [`MessageChannel`] instances, and executes a sender and
/// reader thread to marshal [`Message`] instances between them.
pub(crate) struct ConsoleDriver {
	stdin_channel: MessageChannel,
	stdout_channel: MessageChannel,
}

impl ConsoleDriver {
	/// Initializes a new [`ConsoleDriver`] instance.
	pub fn new() -> Self {
		ConsoleDriver {
			stdin_channel: async_channel::unbounded::<Message>(),
			stdout_channel: async_channel::unbounded::<Message>(),
		}
	}

	/// Retrieves a [`MessageChannel`] from which [`Message`] instances can be received from (i.e., `stdin`) and sent to (i.e., `stdout`).
	pub fn get_message_channel(&self) -> MessageChannel {
		let (sender, _) = self.stdout_channel.clone();
		let (_, receiver) = self.stdin_channel.clone();

		(sender, receiver)
	}

	/// Starts executing the [`ConsoleDriver`] instance.
	pub async fn start(&self, cancel_token: Arc<CancellationToken>) -> Result<(), OperationCanceled> {
		let (sender, _) = self.stdin_channel.clone();
		let (_, receiver) = self.stdout_channel.clone();

		let _sender_task = std::thread::spawn(move || {
			let stdin = stdin();
			let mut stdin = stdin.lock();

			while let Ok(Some(message)) = Message::read(&mut stdin) {
				if sender.send_blocking(message).is_err() {
					break;
				}
			}
		});

		let receiver_task = std::thread::spawn(move || {
			let stdout = stdout();
			let mut stdout = stdout.lock();

			while let Ok(message) = receiver.recv_blocking() {
				if message.write(&mut stdout).is_err() {
					break;
				}
			}
		});

		let (sender, _) = self.stdin_channel.clone();
		let (_, receiver) = self.stdout_channel.clone();

		// Joining to the `receiver_task` will block the current thread. Since this will likely be the main thread it will
		// prevent the async Futures from being driven forward. `spawn_blocking` allows this blocking code to be taken into
		// another thread and returns a `Future` that we can then await.
		task::spawn_blocking(move || {
			cancel_token.run(
				|| {
					sender.close();
					receiver.close();
				},
				|| {
					receiver_task.join().unwrap();

					match cancel_token.is_canceled() {
						true => Err(OperationCanceled),
						_ => Ok(())
					}
				},
			)
		}).await.unwrap()
	}
}
