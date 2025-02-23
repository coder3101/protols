use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use async_lsp::lsp_types::{Url, WorkspaceFolder};
use pkg_config::Config;

use crate::formatter::clang::ClangFormatter;

use super::ProtolsConfig;

const CONFIG_FILE_NAMES: [&str; 2] = [".protols.toml", "protols.toml"];

pub struct WorkspaceProtoConfigs {
    workspaces: HashSet<Url>,
    configs: HashMap<Url, ProtolsConfig>,
    formatters: HashMap<Url, ClangFormatter>,
    protoc_include_prefix: Vec<PathBuf>,
}

impl WorkspaceProtoConfigs {
    pub fn new() -> Self {
        Self {
            workspaces: Default::default(),
            formatters: Default::default(),
            protoc_include_prefix: Config::new()
                .atleast_version("3.0.0")
                .probe("protobuf")
                .map(|l| l.include_paths)
                .unwrap_or_default(),
            configs: Default::default(),
        }
    }

    fn get_config_file_path(wpath: &PathBuf) -> Option<PathBuf> {
        for file in CONFIG_FILE_NAMES {
            let p = Path::new(&wpath).join(file);
            match std::fs::exists(&p) {
                Ok(exists) if exists => return Some(p),
                _ => continue,
            }
        }
        None
    }

    pub fn add_workspace(&mut self, w: &WorkspaceFolder) {
        let Ok(wpath) = w.uri.to_file_path() else {
            return;
        };

        let path = Self::get_config_file_path(&wpath).unwrap_or_default();
        let content = std::fs::read_to_string(path).unwrap_or_default();

        let wr: ProtolsConfig = basic_toml::from_str(&content).unwrap_or_default();
        let fmt = ClangFormatter::new(
            &wr.formatter.clang_format_path,
            wpath.to_str().expect("non-utf8 path"),
        );

        self.workspaces.insert(w.uri.clone());
        self.configs.insert(w.uri.clone(), wr);
        self.formatters.insert(w.uri.clone(), fmt);
    }

    pub fn get_config_for_uri(&self, u: &Url) -> Option<&ProtolsConfig> {
        self.get_workspace_for_uri(u)
            .and_then(|w| self.configs.get(w))
    }

    pub fn get_formatter_for_uri(&self, u: &Url) -> Option<&ClangFormatter> {
        self.get_workspace_for_uri(u)
            .and_then(|w| self.formatters.get(w))
    }

    pub fn get_workspace_for_uri(&self, u: &Url) -> Option<&Url> {
        let upath = u.to_file_path().ok()?;
        self.workspaces
            .iter()
            .find(|&k| upath.starts_with(k.to_file_path().unwrap()))
    }

    pub fn get_include_paths(&self, uri: &Url) -> Option<Vec<PathBuf>> {
        let cfg = self.get_config_for_uri(uri)?;
        let w = self.get_workspace_for_uri(uri)?.to_file_path().ok()?;

        let mut ipath: Vec<PathBuf> = cfg
            .config
            .include_paths
            .iter()
            .map(PathBuf::from)
            .map(|p| if p.is_relative() { w.join(p) } else { p })
            .collect();

        ipath.push(w.to_path_buf());
        ipath.extend_from_slice(&self.protoc_include_prefix);
        Some(ipath)
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Url, WorkspaceFolder};
    use insta::assert_yaml_snapshot;
    use tempfile::tempdir;

    use super::{WorkspaceProtoConfigs, CONFIG_FILE_NAMES};

    #[test]
    fn test_get_for_workspace() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let tmpdir2 = tempdir().expect("failed to create temp2 directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        let mut ws = WorkspaceProtoConfigs::new();
        ws.add_workspace(&WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir.path()).unwrap(),
            name: "Test".to_string(),
        });
        ws.add_workspace(&WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir2.path()).unwrap(),
            name: "Test2".to_string(),
        });

        let inworkspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
        let outworkspace =
            Url::from_file_path(tempdir().unwrap().path().join("out.proto")).unwrap();
        let inworkspace2 = Url::from_file_path(tmpdir2.path().join("foobar.proto")).unwrap();

        assert!(ws.get_config_for_uri(&inworkspace).is_some());
        assert!(ws.get_config_for_uri(&inworkspace2).is_some());
        assert!(ws.get_config_for_uri(&outworkspace).is_none());

        assert!(ws.get_workspace_for_uri(&inworkspace).is_some());
        assert!(ws.get_workspace_for_uri(&inworkspace2).is_some());
        assert!(ws.get_workspace_for_uri(&outworkspace).is_none());

        assert_yaml_snapshot!(ws.get_config_for_uri(&inworkspace).unwrap());
        assert_yaml_snapshot!(ws.get_config_for_uri(&inworkspace2).unwrap());
    }

    #[test]
    fn test_get_formatter_for_uri() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let tmpdir2 = tempdir().expect("failed to create temp2 directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        let mut ws = WorkspaceProtoConfigs::new();
        ws.add_workspace(&WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir.path()).unwrap(),
            name: "Test".to_string(),
        });

        ws.add_workspace(&WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir2.path()).unwrap(),
            name: "Test2".to_string(),
        });

        let inworkspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
        let outworkspace =
            Url::from_file_path(tempdir().unwrap().path().join("out.proto")).unwrap();
        let inworkspace2 = Url::from_file_path(tmpdir2.path().join("foobar.proto")).unwrap();

        assert!(ws.get_formatter_for_uri(&outworkspace).is_none());
        assert_eq!(
            ws.get_formatter_for_uri(&inworkspace).unwrap().path,
            "/usr/bin/clang-format"
        );
        assert_eq!(
            ws.get_formatter_for_uri(&inworkspace2).unwrap().path,
            "clang-format"
        );
    }

    #[test]
    fn test_loading_different_config_files() {
        let tmpdir = tempdir().expect("failed to create temp directory");

        for file in CONFIG_FILE_NAMES {
            let f = tmpdir.path().join(file);
            std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

            let mut ws = WorkspaceProtoConfigs::new();
            ws.add_workspace(&WorkspaceFolder {
                uri: Url::from_directory_path(tmpdir.path()).unwrap(),
                name: "Test".to_string(),
            });

            // check we really loaded the config file
            let workspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
            assert!(ws.get_workspace_for_uri(&workspace).is_some());
        }
    }
}
