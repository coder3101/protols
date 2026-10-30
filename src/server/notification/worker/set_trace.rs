use async_lsp::lsp_types::SetTraceParams;

use super::{Notification, Worker};
use crate::log;

impl Worker {
    pub(super) fn set_trace(&mut self, params: &SetTraceParams) {
        log::update_level(&self.log_handle, params.value);
    }
}

impl From<SetTraceParams> for Notification {
    fn from(params: SetTraceParams) -> Self {
        Notification::SetTrace { params }
    }
}
