use super::RequestId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Request {
	pub id: RequestId,
	pub method: String,
	#[serde(default = "serde_json::Value::default")]
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Response {
	// JSON RPC allows this to be null if it was impossible
	// to decode the request's id. Ignore this special case
	// and just die horribly.
	pub id: RequestId,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub result: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<ResponseError>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseError {
	pub code: i32,
	pub message: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<serde_json::Value>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
	pub method: String,
	#[serde(default = "serde_json::Value::default")]
	#[serde(skip_serializing_if = "serde_json::Value::is_null")]
	pub params: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Message {
	Request(Request),
	Response(Response),
	Notification(Notification),
}
