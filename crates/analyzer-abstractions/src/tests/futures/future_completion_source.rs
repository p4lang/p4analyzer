/// Provides test fixtures for the [`FutureCompletionSource::set_value()`] method.
mod set_value {
	// use async_std::prelude::*;
	use crate::futures::{FutureCompletionSource, FutureCompletionSourceError};

	/// Ensures that [`FutureCompletionSource::set_value()`] fails if the [`FutureCompletionSource`] is already
	/// complete.
	#[test]
	fn returns_err_when_already_complete() {
		let fcs = FutureCompletionSource::<usize, ()>::new_with_value(100);

		assert_eq!(
			fcs.set_value(200),
			Err(FutureCompletionSourceError::Invalid)
		);
	}

	/// Ensures that [`FutureCompletionSource::set_value()`] does not fail when the [`FutureCompletionSource`] is not complete.
	#[test]
	fn accepts_value_when_not_complete() {
		let fcs = FutureCompletionSource::<usize, ()>::new();

		assert_eq!(fcs.set_value(100), Ok(()));
	}

	/// Ensures that the value set via calling [`FutureCompletionSource::set_value)`] is returned to any awaiting future.
	#[async_std::test]
	async fn returns_set_value() {
		let fcs = FutureCompletionSource::<usize, ()>::new();

		assert_eq!(fcs.set_value(100), Ok(()));
		assert_eq!(fcs.future().await, Ok(100));
	}
}


/// Provides test fixtures for the [`FutureCompletionSource::set_err()`] method.
mod set_err {
	use crate::futures::{FutureCompletionSource, FutureCompletionSourceError};

	/// Ensures that [`FutureCompletionSource::set_err()`] fails if the [`FutureCompletionSource`] is already
	/// complete.
	#[test]
	fn returns_err_when_already_complete() {
		let fcs = FutureCompletionSource::<(), usize>::new_with_value(());

		assert_eq!(fcs.set_err(200), Err(FutureCompletionSourceError::Invalid));
	}

	/// Ensures that [`FutureCompletionSource::set_err()`] does not fail when the [`FutureCompletionSource`] is not complete.
	#[test]
	fn accepts_err_when_not_complete() {
		let fcs = FutureCompletionSource::<(), usize>::new();

		assert_eq!(fcs.set_err(100), Ok(()));
	}

	/// Ensures that the error reported via calling [`FutureCompletionSource::set_err()`] is returned to any awaiting future.
	#[async_std::test]
	async fn returns_set_err() {
		let fcs = FutureCompletionSource::<(), usize>::new();

		assert_eq!(fcs.set_err(100), Ok(()));
		assert_eq!(fcs.future().await, Err(100));
	}
}

/// Provides test fixtures for the [`FutureCompletionSource::future()`] method.
mod future {
	use futures::join;
	use thiserror::Error;
	use std::{time::Duration, sync::atomic::{AtomicUsize, Ordering}};
	use crate::futures::{FutureCompletionSource};

	#[derive(Error, Clone, Copy, Debug, PartialEq, Eq)]
	enum MockError {
		#[error("Some error.")]
		Error,
	}

	/// Ensures that if the [`FutureCompletionSource`] is not yet complete, then calls to the [`FutureCompletionSource::future()`]
	/// method will await for the value to be set.
	#[async_std::test]
	async fn awaits_for_value_to_be_set() {
		const EXPECTED_VALUE: usize = 100;

		let fcs = FutureCompletionSource::<usize, MockError>::new();
		let assert = async {
			assert_eq!(fcs.future().await, Ok(EXPECTED_VALUE));
		};
		let set = async {
			async_std::future::timeout(Duration::from_millis(500), async_std::future::pending::<()>()).await.unwrap_err();

			fcs.set_value(EXPECTED_VALUE).unwrap();
		};

		join!(assert, set);
	}

	/// Ensures that when multiple calls to the [`FutureCompletionSource::future()`] method is made prior to
	/// the [`FutureCompletionSource`] becoming complete, then all the awaiting futures will be resolved when a value is set.
	#[async_std::test]
	async fn multiple_futures_await_for_value_to_be_set() {
		const EXPECTED_VALUE: usize = 100;

		let completed_count = AtomicUsize::new(0);
		let fcs = FutureCompletionSource::<usize, MockError>::new();
		let assert1 = async {
			assert_eq!(fcs.future().await, Ok(EXPECTED_VALUE));
			completed_count.fetch_add(1, Ordering::Relaxed);
		};
		let assert2 = async {
			assert_eq!(fcs.future().await, Ok(EXPECTED_VALUE));
			completed_count.fetch_add(1, Ordering::Relaxed);
		};
		let set = async {
			async_std::future::timeout(Duration::from_millis(500), async_std::future::pending::<()>()).await.unwrap_err();

			fcs.set_value(EXPECTED_VALUE).unwrap();
		};

		join!(assert1, assert2, set);

		assert_eq!(completed_count.load(Ordering::Relaxed), 2);
	}
}
