use analyzer_abstractions::lsp_types::{
	notification::Progress as ProgressNotification, request::WorkDoneProgressCreate, NumberOrString, ProgressParams,
	ProgressParamsValue, ProgressToken, WorkDoneProgress, WorkDoneProgressBegin, WorkDoneProgressCreateParams,
	WorkDoneProgressEnd, WorkDoneProgressReport,
};

use super::{request::RequestManager, LspProtocolError};

#[derive(Clone)]
pub(crate) struct ProgressManager {
	request_manager: RequestManager,
	client_supported: bool,
}

impl ProgressManager {
	pub fn new(request_manager: RequestManager, client_supported: bool) -> Self {
		Self { request_manager, client_supported }
	}

	pub async fn begin(&self, title: &str) -> Result<Progress, LspProtocolError> {
		let token = NumberOrString::String(title.into());

		if !self.client_supported {
			return Ok(Progress::new(self.request_manager.clone(), self.client_supported, token));
		}

		let params = WorkDoneProgressCreateParams { token: token.clone() };

		if let Err(_) = self.request_manager.send::<WorkDoneProgressCreate>(params).await {
			return Err(LspProtocolError::TransportError);
		}

		let params = ProgressParams {
			token: token.clone(),
			value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(WorkDoneProgressBegin {
				title: title.into(),
				..Default::default()
			})),
		};

		if let Err(_) = self.request_manager.send_notification::<ProgressNotification>(params).await {
			return Err(LspProtocolError::TransportError);
		}

		Ok(Progress::new(self.request_manager.clone(), self.client_supported, token))
	}
}

pub(crate) struct Progress {
	request_manager: RequestManager,
	client_supported: bool,
	token: ProgressToken,
}

impl Progress {
	fn new(request_manager: RequestManager, client_supported: bool, token: ProgressToken) -> Self {
		Self { request_manager, client_supported, token }
	}

	pub async fn report(&self, message: &str) -> Result<(), LspProtocolError> {
		if !self.client_supported {
			return Ok(());
		}

		let params = ProgressParams {
			token: self.token.clone(),
			value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(WorkDoneProgressReport {
				message: Some(message.into()),
				..Default::default()
			})),
		};

		if let Err(_) = self.request_manager.send_notification::<ProgressNotification>(params).await {
			return Err(LspProtocolError::TransportError);
		}

		Ok(())
	}

	pub async fn end(&self, message: Option<&str>) -> Result<(), LspProtocolError> {
		if !self.client_supported {
			return Ok(());
		}

		let params = ProgressParams {
			token: self.token.clone(),
			value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
				message: message.map(|value| value.into()),
				..Default::default()
			})),
		};

		if let Err(_) = self.request_manager.send_notification::<ProgressNotification>(params).await {
			return Err(LspProtocolError::TransportError);
		}

		Ok(())
	}
}
