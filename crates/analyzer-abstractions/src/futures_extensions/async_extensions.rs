use std::{
	cell::RefCell,
	sync::{Arc, Mutex},
	task::Context,
};

use crate::BoxFuture;
use async_channel::{Receiver, Sender};
use cancellation::{CancellationToken, OperationCanceled};
use futures::{
	task::{waker_ref, ArcWake},
	Future,
};

struct AsyncWork {
	future: Mutex<Option<BoxFuture<'static, ()>>>,
	sender: Sender<Arc<AsyncWork>>,
}

impl AsyncWork {
	pub fn new<T>(future: T, sender: Sender<Arc<AsyncWork>>) -> Self
	where
		T: Future<Output = ()> + Send + Sync + 'static,
	{
		Self { future: Mutex::new(Some(Box::pin(future))), sender }
	}
}

impl ArcWake for AsyncWork {
	fn wake_by_ref(arc_self: &Arc<Self>) {
		if arc_self.sender.is_closed() {
			return;
		}
		let cloned = arc_self.clone();

		arc_self.sender.send_blocking(cloned).unwrap();
	}
}

type WorkChannel = (Sender<Arc<AsyncWork>>, Receiver<Arc<AsyncWork>>);

pub struct AsyncPool;

thread_local!(static WORK_CHANNEL: RefCell<WorkChannel> = RefCell::new(async_channel::unbounded::<Arc<AsyncWork>>()));

impl AsyncPool {
	pub async fn start(cancel_token: Arc<CancellationToken>) -> Result<(), OperationCanceled> {
		let (_, receiver) = WORK_CHANNEL.with(|c| c.borrow().clone());

		while !cancel_token.is_canceled() {
			match receiver.recv().await {
				Ok(work) => {
					let mut future_slot = work.future.lock().unwrap();

					if let Some(mut future) = future_slot.take() {
						let waker = waker_ref(&work);
						let context = &mut Context::from_waker(&*waker);

						if future.as_mut().poll(context).is_pending() {
							*future_slot = Some(future)
						}
					}
				}
				Err(_) => break, // `work_channel` has been closed.
			}
		}

		if cancel_token.is_canceled() {
			return Err(OperationCanceled);
		}

		Ok(())
	}

	pub fn stop() {
		let (_, receiver) = WORK_CHANNEL.with(|c| c.borrow().clone());

		receiver.close();
	}

	pub fn spawn_work<T>(future: T)
	where
		T: Future<Output = ()> + Send + Sync + 'static,
	{
		let (sender, _) = WORK_CHANNEL.with(|c| c.borrow().clone());
		let future = Box::pin(future);
		let work = Arc::new(AsyncWork::new(future, sender.clone()));

		sender.send_blocking(work).unwrap();
	}
}
