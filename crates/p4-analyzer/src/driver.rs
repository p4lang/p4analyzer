use analyzer_host::{json_rpc::message::Message, MessageChannel};
use async_channel::{Receiver, Sender};
#[cfg(test)]
use async_std::task::block_on;
use cancellation::{CancellationToken, OperationCanceled};
use std::{
	io::{stdin, stdout},
	sync::Arc,
};
use tokio::task;

// Module that contains the code for the buffer driver, which is a driver used for testing only
#[cfg(test)]
pub mod buffer_driver;

/// Connects the `stdin` and `stdout` of the process to appropriate [`MessageChannel`] instances, and executes a sender and
/// reader thread to marshal [`Message`] instances between them.
pub struct Driver {
	in_channel: MessageChannel, // Input channel as an intermediate layer for input source -> Analyzer host
	out_channel: MessageChannel, // Output channel as an intermediate layer for Analyzer host -> output source
	source_type: DriverType,    // Handles the connection for input/output of outside layer to intermediate layer
}

#[derive(Clone)]
pub enum DriverType {
	Console,
	#[cfg(test)]
	Buffer(buffer_driver::BufferStruct), // Only want to include this for testing
}

impl DriverType {
	fn to_driver(&self) -> Driver {
		match self {
			DriverType::Console => console_driver(),
			#[cfg(test)]
			DriverType::Buffer(buffer) => buffer_driver::buffer_driver(buffer.clone()),
		}
	}

	fn reader(&self) -> Result<Option<Message>, std::io::Error> {
		match self.clone() {
			DriverType::Console => Message::read(&mut stdin().lock()),
			#[cfg(test)]
			DriverType::Buffer(mut buffer) => block_on(buffer.message_read()),
		}
	}

	fn writer(&self, message: Message) -> Result<(), std::io::Error> {
		match self.clone() {
			DriverType::Console => message.write(&mut stdout()),
			#[cfg(test)]
			DriverType::Buffer(mut buffer) => block_on(buffer.message_write(message)),
		}
	}
}

pub fn console_driver() -> Driver {
	Driver {
		in_channel: async_channel::unbounded::<Message>(),
		out_channel: async_channel::unbounded::<Message>(),
		source_type: DriverType::Console,
	}
}

impl Driver {
	pub fn new(driver_type: DriverType) -> Driver { driver_type.to_driver() }

	/// Retrieves a [`MessageChannel`] from which [`Message`] instances can be received from (i.e., `stdin`) and sent to (i.e., `stdout`).
	pub fn get_message_channel(&self) -> MessageChannel {
		let (sender, _) = self.out_channel.clone();
		let (_, receiver) = self.in_channel.clone();

		(sender, receiver)
	}

	fn sender_task(sender: Sender<Message>, input_source: DriverType) {
		while let Ok(Some(message)) = input_source.reader() {
			if sender.send_blocking(message).is_err() {
				break;
			}
		}
	}

	fn receiver_task(receiver: Receiver<Message>, output_source: DriverType) {
		while let Ok(message) = receiver.recv_blocking() {
			if output_source.writer(message).is_err() {
				break;
			}
		}
	}

	/// Starts executing the [`Driver`] instance.
	pub async fn start(&self, cancel_token: Arc<CancellationToken>) -> Result<(), OperationCanceled> {
		let (sender, _) = self.in_channel.clone();
		let (_, receiver) = self.out_channel.clone();

		let source_type = self.source_type.clone();
		let source_type2 = self.source_type.clone();
		let _sender_task = std::thread::spawn(move || Self::sender_task(sender, source_type));
		let receiver_task = std::thread::spawn(move || Self::receiver_task(receiver, source_type2));

		let (sender, _) = self.in_channel.clone();
		let (_, receiver) = self.out_channel.clone();

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
						_ => Ok(()),
					}
				},
			)
		})
		.await
		.unwrap()
	}
}
