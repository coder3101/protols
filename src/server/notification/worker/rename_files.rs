use std::str::FromStr;

use async_lsp::lsp_types::{RenameFilesParams, Url};

use super::{Notification, Worker};
use crate::server::state::cache::DocumentsGuardExt;
use crate::server::state::source::SourceEntry;

struct RenameTask {
    old_uri: Url,
    new_uri: Url,
    extracted_source: Option<SourceEntry>,
}

impl Worker {
    pub(super) async fn rename_files(&mut self, params: RenameFilesParams) {
        let mut tasks: Vec<RenameTask> = params
            .files
            .iter()
            .filter_map(|r| {
                let old_uri = Url::from_str(&r.old_uri).ok()?;
                let new_uri = Url::from_str(&r.new_uri).ok()?;
                Some(RenameTask {
                    old_uri,
                    new_uri,
                    extracted_source: None,
                })
            })
            .collect();

        if tasks.is_empty() {
            return;
        }

        {
            let (mut documents, mut sources) = self.state.write_lock_all().await;

            for task in &mut tasks {
                documents.cancel_and_remove(&task.old_uri);
                task.extracted_source = sources.remove(&task.old_uri);
            }

            for task in tasks {
                documents.set_dirty(&task.new_uri);

                if let Some(src) = task.extracted_source {
                    sources.insert(task.new_uri, src);
                }
            }
        }
    }
}

impl From<RenameFilesParams> for Notification {
    fn from(params: RenameFilesParams) -> Self {
        Notification::RenameFiles { params }
    }
}
