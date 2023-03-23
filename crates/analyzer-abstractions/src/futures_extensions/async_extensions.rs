use std::{sync::Arc, cell::RefCell};

use futures::{executor::LocalPool, task::{SpawnExt, SpawnError}, Future};

/// A single-threaded pool that runs asynchronous operations (`Future`s) to completion.
pub struct AsyncPool {
	pool: LocalPool
}

thread_local! {
	static CURRENT_ASYNCPOOL: Arc<RefCell<AsyncPool>> = Arc::new(RefCell::new(AsyncPool::new()));
}

impl AsyncPool {
	/// Initializes a new [`AsyncPool`].
	///
	/// (private)
	fn new() -> Self {
		AsyncPool {
			pool: LocalPool::new()
		}
	}

	/// Schedules and executes a supplied `Future` to its completion.
	///
	/// `future` will run in the background, continually being polled by the [`AsyncPool`]'s underlying executor.
	/// If any further `Future`s are created, then they will also be managed by the pool. If you require running
	/// an `async fn` in the background, then [`AsyncPool::run_as_task`] allows you to do that.
	pub fn run_as_task<TFuture>(future: TFuture) -> Result<(), SpawnError>
	where
		TFuture: Future<Output = ()> + Send + 'static
	{
		CURRENT_ASYNCPOOL.with(|instance| {
			let spawner = instance.borrow().pool.spawner();

			spawner.spawn(future)
		})
	}

	/// Blocks the current thread and executes the tasks in the current [`AsyncPool`] until
	/// the given `Future` completes.
	pub fn block_run<TFuture: Future>(future: TFuture) -> TFuture::Output {
		CURRENT_ASYNCPOOL.with(|instance| {
			instance.borrow_mut().pool.run_until(future)
		})
	}
}
