use std::{result::Result, cell::RefCell, sync::{atomic::{AtomicBool, Ordering, AtomicUsize}, Arc}};
use event_listener::Event;
use thiserror::Error;

/// Represents an error that can occur when completing a [`FutureCompletionSource`].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum FutureCompletionSourceError {
	/// The underlying Future has already completed.
	#[error("The underlying Future has already completed.")]
	Invalid
}

type FutureCompletionSourceResult<T> = Result<T, FutureCompletionSourceError>;

/// Represents the producer side of a `Future` unbound to any function, providing access to the
/// consumer side through the [`FutureCompletionSource::future()`] method.
pub struct FutureCompletionSource<T, TError> {
	completed: AtomicBool,
	on_completed: Event,
	awaiting_count: AtomicUsize,
	value: Arc<RefCell<Option<Result<T, TError>>>>,
}

impl<T: Clone, TError: Clone> FutureCompletionSource<T, TError> {
	/// Initializes a new [`FutureCompletionSource`].
	pub fn new() -> Self {
		Self {
			completed: AtomicBool::new(false),
			on_completed: Event::new(),
			awaiting_count: AtomicUsize::new(0),
			value: Arc::new(RefCell::new(None))
		}
	}

	/// Initializes a new [`FutureCompletionSource`] with a given value.
	///
	/// The underlying `Future` will be immediately resolved with `value`, and calling the [`FutureCompletionSource::future()`]
	/// method will complete synchronously returning `value`.
	pub fn new_with_value(value: T) -> Self {
		Self {
			completed: AtomicBool::new(true),
			on_completed: Event::new(),
			awaiting_count: AtomicUsize::new(0),
			value: Arc::new(RefCell::new(Some(Ok(value))))
		}
	}

	/// Resolves the underlying `Future` with a given value.
	pub fn set_value(&self, value: T) -> FutureCompletionSourceResult<()> {
		self.set_inner_value(Ok(value))
	}

	/// Completes the underlying `Future` with a given error.
	pub fn set_err(&self, err: TError) ->  FutureCompletionSourceResult<()> {
		self.set_inner_value(Err(err))
	}

	/// Returns the underlying `Future` created by the current [`FutureCompletionSource`].
	///
	/// This method allows a consumer to access the underlying `Future` that will yield with a value
	/// supplied by the producer when it calls the [`FutureCompletionSource::set_value()`] method;
	/// or complete with an error when called with [`FutureCompletionSource::set_err()`].
	pub async fn future(&self) -> Result<T, TError> {
		let completed = self.completed.load(Ordering::Relaxed);

		// If we have already completed, then simply return the set result.
		if completed {
			return self.value.borrow().as_ref().unwrap().clone();
		}

		// Otherwise, await for an on-completed event before returning the set result.
		self.awaiting_count.fetch_add(1, Ordering::Relaxed); // Increment the awaiting count.
		self.on_completed.listen().await; // Asynchronously wait for the on-completed event.

		return self.value.borrow().as_ref().unwrap().clone()
	}

	fn set_inner_value(&self, result: Result<T, TError>) -> FutureCompletionSourceResult<()> {
		let completed = self.completed.load(Ordering::Relaxed);

		if completed {
			return Err(FutureCompletionSourceError::Invalid);
		}

		// Store the result, set the `completed` state to true and then notify all those that are currently
		// awaiting to resolve their 'Future'.
		self.value.borrow_mut().replace(result);
		self.completed.store(true, Ordering::Relaxed);
		self.on_completed.notify(self.awaiting_count.load(Ordering::Relaxed));

		Ok(())
	}
}
