use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
	/// A binding to the JavaScript built in [`Buffer`] class type that permits dealing with arbitary binary data
	pub type Buffer;

	/// Creates a [`Buffer`] from a [`Uint8Array`] without copying the underlying memory. The given array and the
	/// created buffer will be mapped to the same memory.
	#[wasm_bindgen(static_method_of = Buffer)]
	pub(crate) fn from(array: Uint8Array) -> Buffer;
}

/// Converts a [`Buffer`] into a new vector of `u8`.
pub(crate) fn to_u8_vec(buffer: &Buffer) -> Vec<u8> { Uint8Array::new(buffer).to_vec() }

/// Converts a `u8` slice into a new [`Buffer`].
pub(crate) fn to_buffer(src: &[u8]) -> Buffer {
	let array_buffer = Uint8Array::new_with_length(src.len() as u32);

	array_buffer.copy_from(src);

	Buffer::from(array_buffer)
}
