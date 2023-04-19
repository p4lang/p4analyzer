use analyzer_host::{json_rpc::message::Message, MessageChannel};
use async_channel::{Receiver, Sender};
use cancellation::{CancellationToken, OperationCanceled};
use queues::*;
use std::{
	io::{stdin, stdout, BufReader, self},
	sync::{Arc, RwLock}, net::{TcpStream, SocketAddr}, time::Duration,
};
use tokio::task;

/// Connects the `stdin` and `stdout` of the process to appropriate [`MessageChannel`] instances, and executes a sender and
/// reader thread to marshal [`Message`] instances between them.
pub struct Driver {
	in_channel: MessageChannel,		// Input channel as an intermediate layer for input source -> Anaylzer host
	out_channel: MessageChannel,	// Output channel as an intermediate layer for Anaylzer host -> output source
	source_type: DriveType,			// Handles the connection for input/output of outside layer to intermediate layer
}

#[derive(Clone)]
enum DriveType {
	Console,
	Http(SocketAddr),
	#[cfg(test)] 
	Buffer(BufferStruct),	// Only want to include this for testing
}

pub fn console_driver() -> Driver {
	Driver {
		in_channel: async_channel::unbounded::<Message>(),
		out_channel: async_channel::unbounded::<Message>(),
		source_type: DriveType::Console,
	}
}

pub fn http_driver(ip: SocketAddr) -> Driver {
	Driver {
		in_channel: async_channel::unbounded::<Message>(),
		out_channel: async_channel::unbounded::<Message>(),
		source_type: DriveType::Http(ip),
	}
}

impl Driver {
	/// Retrieves a [`MessageChannel`] from which [`Message`] instances can be received from (i.e., `stdin`) and sent to (i.e., `stdout`).
	pub fn get_message_channel(&self) -> MessageChannel {
		let (sender, _) = self.out_channel.clone();
		let (_, receiver) = self.in_channel.clone();

		(sender, receiver)
	}

	fn sender_task(sender: Sender<Message>,	input_source: DriveType) {
		let match_func = || {
			match input_source.clone() {
				DriveType::Console => Message::read(&mut stdin().lock()),
				DriveType::Http(ip) => Message::read(&mut BufReader::new(TcpStream::connect(ip).unwrap())),	// inefficient as it needs to reconnect for every message
				#[cfg(test)] 
				DriveType::Buffer(mut buffer) => buffer.message_read(),
		}};

		while let Ok(Some(message)) = match_func() {
			if sender.send_blocking(message).is_err() {
				break;
			}
		}
	}

	fn receiver_task(receiver: Receiver<Message>, output_source: DriveType) {
		let message_send = |message: Message| {
			match output_source.clone() {
				DriveType::Console => message.write(&mut stdout()),
				DriveType::Http(ip) => message.write(&mut TcpStream::connect(ip).unwrap()),	// inefficient as it needs to reconnect for every message
				#[cfg(test)] 
				DriveType::Buffer(mut buffer) => buffer.message_write(message),
		}};

		while let Ok(message) = receiver.recv_blocking() {
			if message_send(message).is_err() {
				break;
			}
		}
	}

	/// Starts executing the [`ConsoleDriver`] instance.
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

/// All code below is for DriveType::Buffer(BufferStruct)
/// This type is used for testing but due to needing of talking between threads it has a heavy implementation

#[cfg(test)]
pub fn buffer_driver(buffer: BufferStruct) -> Driver {
	Driver {
		in_channel: async_channel::unbounded::<Message>(),
		out_channel: async_channel::unbounded::<Message>(),
		source_type: DriveType::Buffer(buffer),
	}
}
#[cfg(test)]
struct BufferStructData {
	input_queue: Queue<Message>,	// stores the messages to be send
	output_buffer: Vec<u8>,			// stores the recieved messages(write! doesn't seperate each message)
	output_count: usize,			// Tell you how many messages recieved
	read_queue_count: usize,		// Tells the thread to send a message if var isn't 0
}
#[cfg(test)]
#[derive(Clone)]
pub struct BufferStruct {
	data: Arc<RwLock<BufferStructData>>		// wraps the data in the necessary containers  
}
#[cfg(test)]
impl BufferStruct{
	pub fn new(inputs: Queue<Message>) -> BufferStruct {
		BufferStruct {
			data : Arc::new(RwLock::new(
			BufferStructData {
				input_queue: inputs,
				output_buffer: Vec::new(),
				output_count: 0,
				read_queue_count: 0,
			})),
		}
	}

	pub fn read_queue_blocking(&mut self) -> io::Result<Option<Message>> {
		if self.data.read().unwrap().input_queue.size() == 0 {	// return if empty
			return Ok(None);
		}

		loop {
			match self.data.try_write() {
				Ok(mut guard) =>	{
					if guard.read_queue_count == 0 { drop(guard); std::thread::sleep(Duration::from_millis(1)); continue; }	// Not ready to read, so continue looping
					let ret = Some(guard.input_queue.remove().unwrap());
					guard.read_queue_count -= 1;
					return Ok(ret);
				},
				Err(_) => std::thread::sleep(Duration::from_millis(1)),	// couldn't get lock, so wait
			}
		}
	}
	
	pub fn allow_read(&mut self) {
		self.data.write().unwrap().read_queue_count += 1;
	}

	pub fn get_output_buffer_blocking(&mut self) -> (String, usize) {
		loop {
			match self.data.try_write() {
				Ok(mut guard) => {
					if guard.output_count == 0 {drop(guard); std::thread::sleep(Duration::from_millis(1)); continue;}
					let ret = guard.output_buffer.clone();
					guard.output_buffer.clear();
					let count = guard.output_count;
					guard.output_count = 0;
					return (String::from_utf8(ret).unwrap(), count);
				},
				Err(_) => std::thread::sleep(Duration::from_millis(1)),
			}
		}
	}

	fn message_read(&mut self) -> io::Result<Option<Message>> {
		loop {
			match self.read_queue_blocking() {
				Ok(guard) => return Ok(guard),
				Err(_) => std::thread::sleep(Duration::from_millis(1)),
			}
		}
	} 

	fn message_write(&mut self, message : Message) -> io::Result<()> {
		loop {
			match self.data.try_write() {
				Ok(mut guard) => {	
					guard.output_count += 1;
					return message.write(&mut guard.output_buffer)
				},
				Err(_) => std::thread::sleep(Duration::from_millis(1)),
			}
		}
	}
}	
