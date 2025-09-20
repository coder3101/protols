use std::{
    collections::{HashMap, HashSet},
    env,
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
    cli_include_paths: Vec<PathBuf>,
    init_include_paths: Vec<PathBuf>,
}

impl WorkspaceProtoConfigs {
    pub fn new(cli_include_paths: Vec<PathBuf>) -> Self {
        // Try to find protobuf library and get its include paths
        // Do not emit metadata on stdout as LSP programs can consider
        // it part of spec
        let protoc_include_prefix = Config::new()
            .atleast_version("3.0.0")
            .env_metadata(false)
            .cargo_metadata(false)
            .probe("protobuf")
            .map(|lib| lib.include_paths)
            .unwrap_or_default();

        Self {
            workspaces: HashSet::new(),
            formatters: HashMap::new(),
            configs: HashMap::new(),
            protoc_include_prefix,
            cli_include_paths,
            init_include_paths: Vec::new(),
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
            &wr.config.path.clang_format,
            Some(wpath.to_str().expect("non-utf8 path")),
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

    pub fn set_init_include_paths(&mut self, paths: Vec<PathBuf>) {
        self.init_include_paths = paths;
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

        // Add CLI include paths
        for path in &self.cli_include_paths {
            if path.is_relative() {
                ipath.push(w.join(path));
            } else {
                ipath.push(path.clone());
            }
        }

        // Add initialization include paths
        for path in &self.init_include_paths {
            if path.is_relative() {
                ipath.push(w.join(path));
            } else {
                ipath.push(path.clone());
            }
        }

        ipath.push(w.to_path_buf());
        ipath.extend_from_slice(&self.protoc_include_prefix);
        Some(ipath)
    }

    pub fn no_workspace_mode(&mut self) {
        let wr = ProtolsConfig::default();
        let rp = if cfg!(target_os = "windows") {
            let mut d = String::from("C");
            if let Ok(cdir) = env::current_dir()
                && let Some(drive) = cdir.components().next()
            {
                d = drive.as_os_str().to_string_lossy().to_string()
            }
            format!("{d}://")
        } else {
            String::from("/")
        };
        let uri = match Url::from_file_path(&rp) {
            Err(err) => {
                tracing::error!(?err, "failed to convert path: {rp} to Url");
                return;
            }
            Ok(uri) => uri,
        };

        let fmt = ClangFormatter::new(&wr.config.path.clang_format, None);

        self.workspaces.insert(uri.clone());
        self.configs.insert(uri.clone(), wr);
        self.formatters.insert(uri.clone(), fmt);
    }
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Url, WorkspaceFolder};
    use insta::assert_yaml_snapshot;
    use std::path::PathBuf;
    use tempfile::tempdir;

    use super::{CONFIG_FILE_NAMES, WorkspaceProtoConfigs};

    #[test]
    fn test_get_for_workspace() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let tmpdir2 = tempdir().expect("failed to create temp2 directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        let mut ws = WorkspaceProtoConfigs::new(vec![]);
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

        let mut ws = WorkspaceProtoConfigs::new(vec![]);
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

            let mut ws = WorkspaceProtoConfigs::new(vec![]);
            ws.add_workspace(&WorkspaceFolder {
                uri: Url::from_directory_path(tmpdir.path()).unwrap(),
                name: "Test".to_string(),
            });

            // check we really loaded the config file
            let workspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
            assert!(ws.get_workspace_for_uri(&workspace).is_some());
        }
    }

    #[test]
    fn test_cli_include_paths() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        // Set CLI include paths
        let cli_paths = vec![
            PathBuf::from("/path/to/protos"),
            PathBuf::from("relative/path"),
        ];
        let mut ws = WorkspaceProtoConfigs::new(cli_paths);
        ws.add_workspace(&WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir.path()).unwrap(),
            name: "Test".to_string(),
        });

        let inworkspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
        let include_paths = ws.get_include_paths(&inworkspace).unwrap();

        // Check that CLI paths are included in the result
        assert!(
            include_paths
                .iter()
                .any(|p| p.ends_with("relative/path") || p == &PathBuf::from("/path/to/protos"))
        );

        // The relative path should be resolved relative to the workspace
        let resolved_relative_path = tmpdir.path().join("relative/path");
        assert!(include_paths.contains(&resolved_relative_path));

        // The absolute path should be included as is
        assert!(include_paths.contains(&PathBuf::from("/path/to/protos")));
    }

    #[test]
    fn test_init_include_paths() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        // Set both CLI and initialization include paths
        let cli_paths = vec![PathBuf::from("/cli/path")];
        let init_paths = vec![
            PathBuf::from("/init/path1"),
            PathBuf::from("relative/init/path"),
        ];

        let mut ws = WorkspaceProtoConfigs::new(cli_paths);
        ws.set_init_include_paths(init_paths);
        ws.add_workspace(&WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir.path()).unwrap(),
            name: "Test".to_string(),
        });

        let inworkspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
        let include_paths = ws.get_include_paths(&inworkspace).unwrap();

        // Check that initialization paths are included
        assert!(include_paths.contains(&PathBuf::from("/init/path1")));

        // The relative path should be resolved relative to the workspace
        let resolved_relative_path = tmpdir.path().join("relative/init/path");
        assert!(include_paths.contains(&resolved_relative_path));

        // CLI paths should still be included
        assert!(include_paths.contains(&PathBuf::from("/cli/path")));
    }
}
