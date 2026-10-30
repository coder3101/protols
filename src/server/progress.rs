use async_lsp::ClientSocket;
use async_lsp::lsp_types::WorkDoneProgressCreateParams;
use async_lsp::lsp_types::request::{Request, WorkDoneProgressCreate};
use async_lsp::lsp_types::{
    ProgressParams, ProgressParamsValue, ProgressToken, WorkDoneProgress, WorkDoneProgressBegin,
    WorkDoneProgressEnd, WorkDoneProgressReport, notification::Progress,
};

use crate::config::client::ClientCapabilitiesSummary;

pub struct LspProgressChannel {
    client: ClientSocket,
    token: ProgressToken,
    cancellable: bool,
}

impl LspProgressChannel {
    pub async fn try_create(
        client_capabilities: ClientCapabilitiesSummary,
        client: ClientSocket,
        token: ProgressToken,
        cancellable: bool,
    ) -> Option<Self> {
        if !client_capabilities.supports_work_done_progress {
            return None;
        }

        let request_params = WorkDoneProgressCreateParams {
            token: token.clone(),
        };

        client
            .request::<WorkDoneProgressCreate>(request_params)
            .await
            .inspect_err(|error| {
                tracing::warn!(
                    ?error,
                    method = WorkDoneProgressCreate::METHOD,
                    "Client advertised the method support, but failed to allocate token: {:?}",
                    token
                );
            })
            .ok()?;

        Some(Self {
            client,
            token,
            cancellable,
        })
    }

    pub fn reporter(&self) -> LspProgressReporter {
        LspProgressReporter {
            client: self.client.clone(),
            token: self.token.clone(),
            min_pct: 0,
            max_pct: 100,
            cancellable: self.cancellable,
        }
    }

    pub fn begin(&self, title: &str) {
        let _ = self.client.notify::<Progress>(ProgressParams {
            token: self.token.clone(),
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(WorkDoneProgressBegin {
                title: title.to_string(),
                cancellable: Some(self.cancellable),
                message: None,
                percentage: Some(0),
            })),
        });
    }
}

impl Drop for LspProgressChannel {
    fn drop(&mut self) {
        let _ = self.client.notify::<Progress>(ProgressParams {
            token: self.token.clone(),
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                message: None,
            })),
        });
    }
}

pub trait OptionChannelExt {
    fn begin(&self, title: &str);
    fn reporter(&self) -> Option<LspProgressReporter>;
    fn end(self);
}

impl OptionChannelExt for Option<LspProgressChannel> {
    fn begin(&self, title: &str) {
        if let Some(channel) = self {
            channel.begin(title);
        }
    }

    fn reporter(&self) -> Option<LspProgressReporter> {
        self.as_ref().map(LspProgressChannel::reporter)
    }

    fn end(self) {}
}

#[derive(Clone)]
pub struct LspProgressReporter {
    client: ClientSocket,
    token: ProgressToken,
    cancellable: bool,
    min_pct: u32,
    max_pct: u32,
}

impl LspProgressReporter {
    pub fn with_range(&self, min_pct: u32, max_pct: u32) -> Self {
        Self {
            client: self.client.clone(),
            token: self.token.clone(),
            cancellable: self.cancellable,
            min_pct,
            max_pct,
        }
    }

    pub fn report(&self, percentage: u32, message: &str) {
        let scaled_pct = self.min_pct + (self.max_pct - self.min_pct) * percentage / 100;
        let _ = self.client.notify::<Progress>(ProgressParams {
            token: self.token.clone(),
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                WorkDoneProgressReport {
                    cancellable: Some(self.cancellable),
                    message: Some(message.to_string()),
                    percentage: Some(scaled_pct),
                },
            )),
        });
    }
}

pub trait OptionReporterExt {
    fn report(&self, percentage: u32, message: &str);
    fn with_range(&self, min_pct: u32, max_pct: u32) -> Self;
}

impl OptionReporterExt for Option<LspProgressReporter> {
    fn report(&self, percentage: u32, message: &str) {
        if let Some(reporter) = self {
            reporter.report(percentage, message);
        }
    }

    fn with_range(&self, min_pct: u32, max_pct: u32) -> Self {
        self.as_ref().map(|r| r.with_range(min_pct, max_pct))
    }
}
