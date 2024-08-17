use async_lsp::lsp_types::MarkedString;

use crate::{server::ServerState, utils::split_identifier_package};

impl ServerState {
    pub fn registry_hover(&self, curr_package: &str, identifier: &str) -> Vec<MarkedString> {
        let (mut package, identifier) = split_identifier_package(identifier);
        if package.is_empty() {
            package = curr_package;
        }

        self.trees
            .values()
            .filter(|tree| {
                let content = self.get_content(&tree.uri);
                tree.get_package_name(content.as_bytes())
                    .unwrap_or_default()
                    == package
            })
            .fold(vec![], |mut v, tree| {
                v.extend(tree.hover(identifier, self.get_content(&tree.uri)));
                v
            })
    }
}
