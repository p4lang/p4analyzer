pub trait FilterResult<T, E> {
	type S<A, B>;
	fn filter<F: FnOnce(&T) -> bool>(self, err: E, f: F) -> Self::S<T, E>;
	fn filter_else<F: FnOnce(&T) -> bool, G: FnOnce() -> E>(self, err: G, f: F) -> Self::S<T, E>;
}

impl<T, E> FilterResult<T, E> for Result<T, E> {
	type S<A, B> = Result<T, E>;

	fn filter<F: FnOnce(&T) -> bool>(self, err: E, f: F) -> Self::S<T, E> {
		match self {
			Ok(t) if f(&t) => Ok(t),
			Ok(_) => Err(err),
			Err(e) => Err(e),
		}
	}

	fn filter_else<F: FnOnce(&T) -> bool, G: FnOnce() -> E>(self, err: G, f: F) -> Self::S<T, E> {
		match self {
			Ok(t) if f(&t) => Ok(t),
			Ok(_) => Err(err()),
			Err(e) => Err(e),
		}
	}
}
