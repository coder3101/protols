use super::{Notification, Worker};

impl Worker {
    pub(super) fn exit(&mut self) {
        if self.shutdown_token.is_cancelled() {
            tracing::info!(
                "Received exit notification after shutdown. Terminating background worker cleanly."
            );
        } else {
            tracing::warn!(
                "Received exit notification WITHOUT shutdown! Process state is inconsistent."
            );
        }
    }
}

impl From<()> for Notification {
    fn from(_params: ()) -> Self {
        Notification::Exit
    }
}
