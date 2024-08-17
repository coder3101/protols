use async_lsp::lsp_types::MarkedString;

use crate::{state::ProtoLanguageState, utils::split_identifier_package};

impl ProtoLanguageState {
    pub fn hover(&self, curr_package: &str, identifier: &str) -> Vec<MarkedString> {
        let (mut package, identifier) = split_identifier_package(identifier);
        if package.is_empty() {
            package = curr_package;
        }

        self.get_trees_for_package(package)
            .into_iter()
            .fold(vec![], |mut v, tree| {
                v.extend(tree.hover(identifier, self.get_content(&tree.uri)));
                v
            })
    }
}

#[cfg(test)]
mod test {
    use crate::state::ProtoLanguageState;

    #[test]
    fn workspace_test_hover() {
    }
}
