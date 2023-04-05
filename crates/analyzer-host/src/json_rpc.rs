pub mod message;

use message::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
	fmt::{self, Display},
	io::{self, BufRead, Write}
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]

pub struct RequestId(IdRepr);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(untagged)]

enum IdRepr {
	I32(i32),
	String(String)
}

impl From<i32> for RequestId {
	fn from(id: i32) -> RequestId { RequestId(IdRepr::I32(id)) }
}

impl From<String> for RequestId {
	fn from(id: String) -> RequestId { RequestId(IdRepr::String(id)) }
}

impl fmt::Display for RequestId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

		match &self.0 {
			IdRepr::I32(it) => fmt::Display::fmt(it, f),
			// Use debug here, to make it clear that `92` and `"92"` are
			// different, and to reduce WTF factor if the sever uses `" "` as an
			// ID.
			IdRepr::String(it) => fmt::Debug::fmt(it, f)
		}
	}
}

#[derive(Clone, Copy)]

pub enum ErrorCode {
	ServerNotInitialized = -32002,
	InvalidRequest = -32600,
	InvalidParams = -32602,
	InternalError = -32603
}

impl Message {
	pub fn read(r: &mut impl BufRead) -> io::Result<Option<Message>> { Message::buffered_read(r) }

	pub fn write(self, w: &mut impl Write) -> io::Result<()> { self.buffered_write(w) }

	fn buffered_read(r: &mut dyn BufRead) -> io::Result<Option<Message>> {

		let text = match read_msg_text(r)? {
			None => return Ok(None),
			Some(text) => text
		};

		let msg = serde_json::from_str(&text)?;

		Ok(Some(msg))
	}

	fn buffered_write(self, w: &mut dyn Write) -> io::Result<()> {

		#[derive(Serialize)]

		struct JsonRpc {
			jsonrpc: &'static str,
			#[serde(flatten)]
			msg: Message
		}

		let text = serde_json::to_string(&JsonRpc { jsonrpc: "2.0", msg: self })?;

		write_msg_text(w, &text)
	}
}

impl Display for Message {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

		match self {
			Message::Request(req) => write!(f, "{}:{}", req.method, req.id)?,
			Message::Notification(notification) => write!(f, "{}", notification.method)?,
			Message::Response(resp) => write!(f, ":{}", resp.id)?
		}

		Ok(())
	}
}

fn read_msg_text(inp: &mut dyn BufRead) -> io::Result<Option<String>> {

	fn invalid_data(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {

		io::Error::new(io::ErrorKind::InvalidData, error)
	}

	macro_rules! invalid_data {
		($($tt:tt)*) => (invalid_data(format!($($tt)*)))
	}

	let mut size = None;

	let mut buf = String::new();

	loop {

		buf.clear();

		if inp.read_line(&mut buf)? == 0 {

			return Ok(None);
		}

		if !buf.ends_with("\r\n") {

			return Err(invalid_data!("malformed header: {:?}", buf));
		}

		let buf = &buf[..buf.len() - 2];

		if buf.is_empty() {

			break;
		}

		let mut parts = buf.splitn(2, ": ");

		let header_name = parts.next().unwrap();

		let header_value = parts.next().ok_or_else(|| invalid_data!("malformed header: {:?}", buf))?;

		if header_name == "Content-Length" {

			size = Some(header_value.parse::<usize>().map_err(invalid_data)?);
		}
	}

	let size: usize = size.ok_or_else(|| invalid_data!("no Content-Length"))?;

	let mut buf = buf.into_bytes();

	buf.resize(size, 0);

	inp.read_exact(&mut buf)?;

	let buf = String::from_utf8(buf).map_err(invalid_data)?;

	// log::debug!("< {}", buf);?
	Ok(Some(buf))
}

fn write_msg_text(out: &mut dyn Write, msg: &str) -> io::Result<()> {

	// log::debug!("> {}", msg);
	write!(out, "Content-Length: {}\r\n\r\n", msg.len())?;

	out.write_all(msg.as_bytes())?;

	out.flush()?;

	Ok(())
}

impl Request {
	pub fn new<TParams: Serialize>(id: RequestId, method: String, params: TParams) -> Self {

		Request { id, method, params: serde_json::to_value(params).unwrap() }
	}
}

impl Response {
	/// Create a new [`Response`] based on given data.

	pub fn new<TResult: Serialize>(id: RequestId, data: TResult) -> Self {

		Response { id, result: Some(serde_json::to_value(data).unwrap()), error: None }
	}

	/// Creates a new [`Response`] that contains an error based on a given error code and message.

	pub fn new_error(id: RequestId, code: i32, message: &str) -> Self {

		Response { id, result: None, error: Some(ResponseError { code, message: String::from(message), data: None }) }
	}
}

impl Notification {
	pub fn new<T: Serialize>(method: &'static str, data: T) -> Self {

		Self { method: method.to_string(), params: serde_json::to_value(data).unwrap() }
	}
}

/// An error that is the result of a failed attempt to serialize an object into a JSON value.
#[derive(thiserror::Error, Clone, Copy, Debug)]

pub struct SerializeError;

impl std::fmt::Display for SerializeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Failed to serialize object into JSON.") }
}

/// An error that is the result of a failed attempt to deserialize a JSON value.
#[derive(thiserror::Error, Clone, Copy, Debug)]

pub struct DeserializeError;

impl std::fmt::Display for DeserializeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Failed to deserialize object from JSON.") }
}

/// Serializes an object into a JSON value.

pub fn to_json<T: Serialize>(item: T) -> Result<serde_json::Value, SerializeError> {

	let value = serde_json::to_value(item).map_err(|_| SerializeError)?;

	Ok(value)
}

/// Deserializes a JSON value to an object.

pub fn from_json<T: DeserializeOwned>(json: &serde_json::Value) -> Result<T, DeserializeError> {

	let res = serde_json::from_value(json.clone()).map_err(|_| DeserializeError)?;

	Ok(res)
}
