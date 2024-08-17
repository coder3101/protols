use async_lsp::lsp_types::Location;

use crate::{state::ProtoLanguageState, utils::split_identifier_package};

impl ProtoLanguageState {
    pub fn definition(&self, curr_package: &str, identifier: &str) -> Vec<Location> {
        let (mut package, identifier) = split_identifier_package(identifier);
        if package.is_empty() {
            package = curr_package;
        }
        self.get_trees_for_package(package)
            .into_iter()
            .fold(vec![], |mut v, tree| {
                v.extend(tree.definition(identifier, self.get_content(&tree.uri)));
                v
            })
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn workspace_test_definition() {}
}
