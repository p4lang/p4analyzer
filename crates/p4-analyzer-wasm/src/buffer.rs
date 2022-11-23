use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
	pub type Buffer;

	#[wasm_bindgen(method, getter)]
	pub(crate) fn buffer(this: &Buffer) -> ArrayBuffer;

	#[wasm_bindgen(method, getter, js_name = byteOffset)]
	fn byte_offset(this: &Buffer) -> u32;

	#[wasm_bindgen(method, getter)]
	fn length(this: &Buffer) -> u32;

	#[wasm_bindgen(static_method_of = Buffer)]
	fn from(array: Uint8Array) -> Buffer;
}

/// Converts an ArrayBuffer to a new vector of `u8`.
pub(crate) fn to_u8_vec(buffer: &ArrayBuffer) -> Vec<u8> {
	Uint8Array::new(buffer).to_vec()
}

/// Converts a vector of `u8` into a new Buffer.
pub(crate) fn to_buffer(vec: &Vec<u8>) -> Buffer {
	let array_buffer = Uint8Array::new_with_length(vec.len() as u32);

	array_buffer.copy_from(vec);

	Buffer::from(array_buffer)
}
