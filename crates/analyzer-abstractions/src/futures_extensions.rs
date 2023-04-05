use core::fmt::Debug;
use event_listener::Event;
use std::{
	result::Result,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, RwLock,
	},
	task::Poll,
};
use thiserror::Error;

pub mod async_extensions;

/// Represents an error that can occur when completing a [`FutureCompletionSource`].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum FutureCompletionSourceError {
	/// The underlying Future has already completed.
	#[error("The underlying Future has already completed.")]
	Invalid,
}

/// Represents the producer side of a `Future` unbound to any function, providing access to the
/// consumer side through the [`FutureCompletionSource::future()`] method.
#[derive(Clone)]
pub struct FutureCompletionSource<T, TError> {
	state: Arc<State<T, TError>>,
}

impl<T, TError> FutureCompletionSource<T, TError>
where
	T: Clone + Debug,
	TError: Copy + Debug,
{
	/// Initializes a new [`FutureCompletionSource`].
	pub fn new() -> Self {
		Self {
			state: Arc::new(State {
				completed: AtomicBool::new(false),
				on_completed: Event::new(),
				value: Arc::new(RwLock::new(None)),
			}),
		}
	}

	/// Initializes a new [`FutureCompletionSource`] with a given value.
	///
	/// The underlying `Future` will be immediately resolved with `value`, and calling the [`FutureCompletionSource::future()`]
	/// method will complete synchronously returning `value`.
	pub fn new_with_value(value: T) -> Self {
		Self {
			state: Arc::new(State {
				completed: AtomicBool::new(true),
				on_completed: Event::new(),
				value: Arc::new(RwLock::new(Some(Ok(value)))),
			}),
		}
	}

	/// Resolves the underlying `Future` with a given value.
	pub fn set_value(&self, value: T) -> FutureCompletionSourceResult<()> { self.set_inner_value(Ok(value)) }

	/// Completes the underlying `Future` with a given error.
	pub fn set_err(&self, err: TError) -> FutureCompletionSourceResult<()> { self.set_inner_value(Err(err)) }

	/// Returns the underlying `Future` created by the current [`FutureCompletionSource`].
	///
	/// This method allows a consumer to access the underlying `Future` that will yield with a value
	/// supplied by the producer when it calls the [`FutureCompletionSource::set_value()`] method;
	/// or complete with an error when called with [`FutureCompletionSource::set_err()`].
	pub async fn future(&self) -> Result<T, TError> {
		if let Poll::Ready(value) = self.state() {
			return value;
		}

		// Otherwise, await for an on-completed event before returning the set result.
		self.state.on_completed.listen().await; // Asynchronously wait for the on-completed event.

		if let Poll::Ready(value) = self.state() {
			return value;
		}

		unreachable!()
	}

	/// Retrieves the state of the current [`FutureCompletionSource`].
	///
	/// If [`Poll::Pending`] is returned then the producing side of the Future has not yet set a value or an error.
	pub fn state(&self) -> Poll<Result<T, TError>> {
		match self.state.completed.load(Ordering::Relaxed) {
			true => {
				let reader = self.state.value.read().unwrap();
				let result = reader.as_ref().unwrap();

				Poll::Ready(match result {
					Ok(value) => Ok(value.clone()),
					Err(err) => Err(*err),
				})
			}
			false => Poll::Pending,
		}
	}

	#[inline(always)]
	fn set_inner_value(&self, result: Result<T, TError>) -> FutureCompletionSourceResult<()> {
		let completed = self.state.completed.load(Ordering::Relaxed);

		if completed {
			return Err(FutureCompletionSourceError::Invalid);
		}

		// Store the result, set the `completed` state to true and then notify all those that are currently
		// awaiting to resolve their 'Future'.
		let mut writer = self.state.value.write().unwrap();

		writer.replace(result);
		self.state.completed.store(true, Ordering::Relaxed);
		self.state.on_completed.notify(usize::MAX); // Notify all awaiting.

		Ok(())
	}
}

type FutureCompletionSourceResult<T> = Result<T, FutureCompletionSourceError>;

/// Encapsulates the internal (clonable) state of a [`FutureCompletionSource`].
///
/// (private)
struct State<T, TError> {
	completed: AtomicBool,
	on_completed: Event,
	value: Arc<RwLock<Option<Result<T, TError>>>>,
}
