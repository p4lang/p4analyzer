/// BufferStruct is a type of Driver that uses internal Buffers and a counter to limit when the Driver recieves Messages
/// This is useful for testings and tools that require more control of the program: LSIF generation & Blackbox testing
/// BufferStruct wraps BufferStructData in Arc<> due to the strict requirement of ownership of the data and it's only handled through BufferStruct API
///
/// The General premise is adding messages you want the Driver to recieve to the input_queue through BufferStruct::new() or BufferStruct::add_to_queue_blocking()
/// Then call BufferStruct::allow_read_blocking() or simalar functions to let the Driver read the Messages in the Queue
/// Then use BufferStruct::get_output_buffer(expected_messages) which will wait for the output_buffer size to be greater than expected_messages then return the Messages
///
/// BufferStruct require AnalyzerHost::start() and Driver::start() running in Async or on a seperate thread at the same time
/// BufferStruct just allows for the control of Message sending and receieving, the logic is done in those 2 Futures
///
/// As BufferStruct is a type of Driver/subset of it, some of the code in this file is only for the Driver, that code shouldn't hold Locks for any longer than needed to avoid deadlocks
use super::*;
use core::panic;
use queues::*;
use std::{
	io,
	sync::{Mutex, RwLock},
	time::Duration,
};

pub fn buffer_driver(buffer: Arc<BufferStruct>) -> Driver {
	Driver {
		in_channel: async_channel::unbounded::<Message>(),
		out_channel: async_channel::unbounded::<Message>(),
		source_type: DriverType::Buffer(buffer),
	}
}

pub struct BufferStructData {
	input_queue: Queue<Message>, // stores the messages to be sent
	output_buffer: Vec<Vec<u8>>, // stores the received messages
}

#[derive(Clone)]
pub struct BufferStruct {
	data: Arc<RwLock<BufferStructData>>, // wraps the data in the necessary containers
	read_queue_count: Arc<Mutex<usize>>, // reading the counter is a major blocking point for read_queue() so given it's own Lock
}

impl BufferStruct {
	pub fn new(inputs: Queue<Message>) -> BufferStruct {
		BufferStruct {
			data: Arc::new(RwLock::new(BufferStructData { input_queue: inputs, output_buffer: Vec::new() })),
			read_queue_count: Arc::new(Mutex::new(0)),
		}
	}

	/// Only the Driver needs this code
	async fn read_queue(&self) -> io::Result<Option<Message>> {
		loop {
			match self.read_queue_count.try_lock() {
				Ok(mut guard) => {
					if *guard == 0 || self.data.read().unwrap().input_queue.size() == 0 {
						drop(guard);
						async_std::task::sleep(Duration::from_millis(1)).await;
						continue;
					} // Not ready to read, so continue looping

					*guard -= 1; // confirm we're doing a Read
					drop(guard); // drop lock to avoid dead locks

					// now wait for data lock because we're been give the all clear
					let mut lock = self.data.write().unwrap();

					let ret = Some(lock.input_queue.remove().unwrap());
					return Ok(ret);
				}
				Err(_) => async_std::task::sleep(Duration::from_millis(1)).await, // couldn't get lock, so wait
			}
		}
	}

	/// Allows Driver to read the next message in the queue
	/// It will also hand over the concurrency thread handler to the next Future
	pub async fn allow_read_blocking(&self) {
		*(self.read_queue_count.lock().unwrap()) += 1;
		// Gives up thread handle to another concurrent async function
		// TODO: check this works trick works
		async_std::task::sleep(Duration::from_millis(0)).await;
	}

	/// Does the same as allow_read_blocking() but lets you specify how many it reads
	pub async fn allow_any_read_blocking(&self, size: usize) {
		*(self.read_queue_count.lock().unwrap()) += size;
		async_std::task::sleep(Duration::from_millis(0)).await; // see comment above
	}

	/// Allows Driver to read every message in the Queue & hands over concurrency handler
	pub async fn allow_all_read_blocking(&self) {
		*(self.read_queue_count.lock().unwrap()) = self.data.read().unwrap().input_queue.size();
		async_std::task::sleep(Duration::from_millis(0)).await; // see comment above
	}

	/// Call allow_read_blocking() or any assoiate functions, before trying to read output buffer
	/// Will block until the number of messages recieved is greater or equal to the argument passed to it
	/// A manual  
	pub async fn get_output_buffer(&self, size: usize) -> Vec<String> {
		loop {
			match self.data.try_write() {
				Ok(mut guard) => {
					if guard.output_buffer.len() < size {
						drop(guard);
						async_std::task::sleep(Duration::from_millis(1)).await;
						continue;
					}
					let mut ret = Vec::<String>::new();
					for elm in guard.output_buffer.clone() {
						ret.push(String::from_utf8(elm).unwrap());
					}
					guard.output_buffer.clear();
					return ret;
				}
				Err(_) => async_std::task::sleep(Duration::from_millis(1)).await,
			}
		}
	}

	// TODO: Change visablity for these 2 helper functions
	// ! Only use in Driver
	pub async fn message_read(&self) -> io::Result<Option<Message>> {
		loop {
			match self.read_queue().await {
				Ok(guard) => return Ok(guard),
				Err(_) => async_std::task::sleep(Duration::from_millis(1)).await,
			}
		}
	}

	// ! Only use in Driver
	pub async fn message_write(&self, message: Message) -> io::Result<()> {
		loop {
			match self.data.try_write() {
				Ok(mut guard) => {
					let mut buf = Vec::<u8>::new();
					let res = message.write(&mut buf);
					guard.output_buffer.push(buf);
					return res;
				}
				Err(_) => async_std::task::sleep(Duration::from_millis(1)).await,
			}
		}
	}

	/// Simply adds the argument to the Queue
	/// Is blocking as it needs ownership of the RwLock
	pub fn add_to_queue_blocking(&self, mut add: Queue<Message>) {
		let mut lock = self.data.write().unwrap();
		while let Ok(mess) = add.remove() {
			lock.input_queue.add(mess).unwrap();
		}
	}

	/// Helper function that adds messages to the queue and tell the Driver that the whole queue is ready for reading  
	pub async fn send_messages(&self, add: Queue<Message>) {
		let size = add.size();
		self.add_to_queue_blocking(add);
		self.allow_all_read_blocking().await;
		self.wait_for_process_messages().await;
	}

	/// Helper function that will add the 1st argument to the queue and then wait for result to be returned
	/// Need to specify how many response messages are expected in 2nd argument
	/// Potential for hanging the program if LSP crashes as it will wait for messages that never come
	pub async fn send_recieve_messages(&self, add: Queue<Message>, expected_messages: usize) -> Vec<String> {
		self.send_messages(add).await;
		self.wait_for_process_messages().await;
		self.get_output_buffer(expected_messages).await
	}

	/// As get_output_buffer() returns everything in the buffer, past and present, this function allows the past buffer to be cleared
	pub fn clear_output_buffer(&self) { self.data.write().unwrap().output_buffer.clear(); }

	/// Removes all incoming messages from the Queue and stops any further message reads by Driver
	pub fn clear_message_buffer(&self) {
		while self.data.write().unwrap().input_queue.remove().is_ok() {}
		*(self.read_queue_count.lock().unwrap()) = 0;
	}

	pub fn clear_both_buffers(&self) {
		self.clear_message_buffer();
		self.clear_output_buffer();
	}

	pub async fn wait_for_process_messages(&self) {
		loop {
			match self.data.try_write() {
				Ok(mut guard) => {
					if guard.input_queue.size() == 0 {
						return;
					}

					drop(guard);
					async_std::task::sleep(Duration::from_millis(1)).await
				}
				Err(_) => async_std::task::sleep(Duration::from_millis(1)).await,
			}
		}
	}
}
