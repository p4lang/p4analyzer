pub mod message;

use message::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
	fmt,
	io::{self, BufRead, Write},
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct RequestId(IdRepr);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(untagged)]
enum IdRepr {
	I32(i32),
	String(String),
}

impl From<i32> for RequestId {
	fn from(id: i32) -> RequestId {
		RequestId(IdRepr::I32(id))
	}
}

impl From<String> for RequestId {
	fn from(id: String) -> RequestId {
		RequestId(IdRepr::String(id))
	}
}

impl fmt::Display for RequestId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match &self.0 {
			IdRepr::I32(it) => fmt::Display::fmt(it, f),
			// Use debug here, to make it clear that `92` and `"92"` are
			// different, and to reduce WTF factor if the sever uses `" "` as an
			// ID.
			IdRepr::String(it) => fmt::Debug::fmt(it, f),
		}
	}
}

pub enum ErrorCode {
	ServerNotInitialized = -32002,
}

impl Message {
	pub fn read(r: &mut impl BufRead) -> io::Result<Option<Message>> {
		Message::buffered_read(r)
	}

	pub fn write(self, w: &mut impl Write) -> io::Result<()> {
		self.buffered_write(w)
	}

	fn buffered_read(r: &mut dyn BufRead) -> io::Result<Option<Message>> {
		let text = match read_msg_text(r)? {
			None => return Ok(None),
			Some(text) => text,
		};
		let msg = serde_json::from_str(&text)?;
		Ok(Some(msg))
	}

	fn buffered_write(self, w: &mut dyn Write) -> io::Result<()> {
		#[derive(Serialize)]
		struct JsonRpc {
			jsonrpc: &'static str,
			#[serde(flatten)]
			msg: Message,
		}
		let text = serde_json::to_string(&JsonRpc {
			jsonrpc: "2.0",
			msg: self,
		})?;
		write_msg_text(w, &text)
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
		let header_value = parts
			.next()
			.ok_or_else(|| invalid_data!("malformed header: {:?}", buf))?;
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
	/// Returns `true` if the current request is the `'initialize'` request.
	pub(crate) fn is_initialize(&self) -> bool {
		self.method == "initialize"
	}
}

impl Response {
	/// Create a new [`Response`] based on given data.
	pub fn new<TResult: Serialize>(id: RequestId, data: TResult) -> Self {
		Response {
			id,
			result: Some(serde_json::to_value(data).unwrap()),
			error: None,
		}
	}

	/// Creates a new [`Response`] that contains an error based on a given error code and message.
	pub fn new_error(id: RequestId, code: i32, message: &str) -> Self {
		Response {
			id,
			result: None,
			error: Some(ResponseError {
				code,
				message: String::from(message),
				data: None,
			}),
		}
	}
}

impl Notification {
	/// Returns `true` if the current notification is the `'exit'` notification.
	pub(crate) fn is_exit(&self) -> bool {
		self.method == "exit"
	}
}

/// An error that is the result of a failed attempt to deserialize a JSON object.
pub type DeserializeError = Box<dyn std::error::Error + Send + Sync>;

/// Deserializes a JSON value.
pub fn from_json<T: DeserializeOwned>(
	what: &'static str,
	json: &serde_json::Value,
) -> Result<T, DeserializeError> {
	let res = serde_json::from_value(json.clone())
		.map_err(|e| format!("Error deserializing '{}': {}; {}", what, e, json))?;

	Ok(res)
}
