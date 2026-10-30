use std::sync::Arc;

use super::DocumentVersion;

#[derive(Clone)]
pub struct SourceEntry {
    pub version: DocumentVersion,
    pub content: Arc<String>,
}
