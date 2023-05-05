// Code is in a seperate folder as all code in here is intended for test only code
// Is a child module to driver, that is wrapped in a #[cfg(test)] tag

use super::*;
use queues::*;
use std::{
	io,
	sync::{Mutex, RwLock},
	time::Duration,
};

pub fn buffer_driver(buffer: BufferStruct) -> Driver {
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

	pub async fn read_queue(&mut self) -> io::Result<Option<Message>> {
		loop {
			match self.read_queue_count.try_lock() {
				Ok(mut guard) => {
					if *guard == 0 {
						drop(guard);
						async_std::task::sleep(Duration::from_millis(1)).await;
						continue;
					} // Not ready to read, so continue looping

					*guard -= 1; // confirm we're doing a Read
					drop(guard); // drop lock to avoid dead locks
			 // now wait for data lock because we're been give the all clear
					let mut lock = self.data.write().unwrap();

					if lock.input_queue.size() == 0 {
						// return if empty
						return Ok(None);
					}

					let ret = Some(lock.input_queue.remove().unwrap());
					return Ok(ret);
				}
				Err(_) => async_std::task::sleep(Duration::from_millis(1)).await, // couldn't get lock, so wait
			}
		}
	}

	pub fn allow_read_blocking(&mut self) { *self.read_queue_count.lock().unwrap() += 1; }

	pub async fn get_output_buffer(&mut self) -> Vec<String> {
		loop {
			match self.data.try_write() {
				Ok(mut guard) => {
					if guard.output_buffer.len() == 0 {
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

	pub async fn message_read(&mut self) -> io::Result<Option<Message>> {
		loop {
			match self.read_queue().await {
				Ok(guard) => return Ok(guard),
				Err(_) => async_std::task::sleep(Duration::from_millis(1)).await,
			}
		}
	}

	pub async fn message_write(&mut self, message: Message) -> io::Result<()> {
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

	// Not advised as if driver attempt a read when emtpy, it will close
	// Also requires lock to remain thread safe
	// proper way is to create a Queue with items already in stack, and pass to BufferStruct::new()
	fn add_to_queue_blocking(&mut self, mut add: Queue<Message>) {
		let mut lock = self.data.write().unwrap();
		while let Ok(mess) = add.remove() {
			lock.input_queue.add(mess).unwrap();
		}
	}
}
