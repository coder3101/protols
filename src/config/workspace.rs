use std::{
    collections::{HashMap, HashSet},
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_lsp::lsp_types::{Url, WorkspaceFolder};
use futures::StreamExt;
use pkg_config::Config;

use crate::{config::ProtolsConfig, formatter::clang::ClangFormatter, utils::OutermostPaths};

const CONFIG_FILE_NAMES: [&str; 2] = [".protols.toml", "protols.toml"];

pub struct WorkspaceProtoConfigs {
    workspaces: HashSet<Url>,
    configs: HashMap<Url, ProtolsConfig>,
    formatters: HashMap<Url, Arc<ClangFormatter>>,
    protoc_include_prefix: Vec<PathBuf>,
    cli_include_paths: Vec<PathBuf>,
    init_include_paths: Vec<PathBuf>,
    fallback_include_path: Option<PathBuf>,
}

impl WorkspaceProtoConfigs {
    pub fn new(cli_include_paths: Vec<PathBuf>, fallback_include_path: Option<PathBuf>) -> Self {
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
            fallback_include_path,
            protoc_include_prefix,
            cli_include_paths,
            init_include_paths: Vec::new(),
        }
    }

    pub(super) async fn get_config_file_path(wroot: &PathBuf) -> Option<PathBuf> {
        for file in CONFIG_FILE_NAMES {
            let p = Path::new(&wroot).join(file);

            if tokio::fs::try_exists(&p).await.unwrap_or_default() {
                return Some(p);
            }
        }
        None
    }

    pub fn add_workspace_folder(&mut self, data: ConfiguredWorkspaceFolder) {
        self.workspaces.insert(data.uri.clone());
        self.configs.insert(data.uri.clone(), data.config);
        self.formatters.insert(data.uri, Arc::new(data.formatter));
    }

    pub fn remove_workspace_folder(&mut self, wf: &WorkspaceFolder) {
        let uri = &wf.uri;

        self.workspaces.remove(uri);
        self.configs.remove(uri);
        self.formatters.remove(uri);
    }

    pub fn get_config_for_uri(&self, u: &Url) -> Option<&ProtolsConfig> {
        self.get_workspace_for_uri(u)
            .and_then(|w| self.configs.get(w))
    }

    pub fn get_formatter_for_uri(&self, u: &Url) -> Option<Arc<ClangFormatter>> {
        self.get_workspace_for_uri(u)
            .and_then(|w| self.formatters.get(w))
            .cloned()
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

        ipath.push(w.clone());
        ipath.extend_from_slice(&self.protoc_include_prefix);
        ipath.extend_from_slice(self.fallback_include_path.as_slice());
        Some(ipath)
    }

    pub fn get_workspaces(&self) -> Vec<&Url> {
        self.workspaces.iter().collect()
    }

    pub fn no_workspace_mode(&mut self) {
        let wr = ProtolsConfig::default();
        let root_path = env::current_dir()
            .ok()
            .and_then(|cdir| cdir.ancestors().last().map(Path::to_path_buf))
            .unwrap_or_else(|| {
                if cfg!(target_os = "windows") {
                    r"C:\".into()
                } else {
                    "/".into()
                }
            });
        let uri = match Url::from_file_path(&root_path) {
            Err(err) => {
                tracing::error!(?err, ?root_path, "failed to convert root path to Url");
                return;
            }
            Ok(uri) => uri,
        };

        let cmd = wr.config.path.clang_format.clone().into_os_string();
        let fmt = ClangFormatter::new(cmd, None);

        self.workspaces.insert(uri.clone());
        self.configs.insert(uri.clone(), wr);
        self.formatters.insert(uri.clone(), Arc::new(fmt));
    }

    pub fn get_all_indexing_paths(&self) -> OutermostPaths {
        let workspace_paths = self
            .workspaces
            .iter()
            .filter_map(|url| url.to_file_path().ok());

        let include_paths = self
            .workspaces
            .iter()
            .filter_map(|url| self.get_include_paths(url))
            .flatten();

        OutermostPaths::from_iter(workspace_paths.chain(include_paths))
    }
}

pub(crate) struct ConfiguredWorkspaceFolder {
    pub uri: Url,
    config: ProtolsConfig,
    formatter: ClangFormatter,
}

const MAX_CONCURRENT_DISK_SCANS: usize = 4;

pub async fn configure_workspace_folders(
    folders: &[WorkspaceFolder],
) -> Vec<ConfiguredWorkspaceFolder> {
    if folders.is_empty() {
        return Vec::new();
    }

    futures::stream::iter(folders)
        .map(configure_workspace_folder)
        .buffer_unordered(MAX_CONCURRENT_DISK_SCANS)
        .filter_map(|res| async move { res })
        .collect()
        .await
}

async fn configure_workspace_folder(wf: &WorkspaceFolder) -> Option<ConfiguredWorkspaceFolder> {
    let wfpath = wf.uri.to_file_path().ok()?;
    let config = parse_config_file(&wfpath).await;
    let cmd = config.config.path.clang_format.clone().into_os_string();

    let formatter = ClangFormatter::new(cmd, Some(wfpath));

    let uri = wf.uri.clone();
    Some(ConfiguredWorkspaceFolder {
        uri,
        config,
        formatter,
    })
}

#[inline]
async fn parse_config_file(path: &PathBuf) -> ProtolsConfig {
    let Some(config_path) = WorkspaceProtoConfigs::get_config_file_path(path).await else {
        return ProtolsConfig::default();
    };

    let Ok(bytes) = tokio::fs::read(config_path).await else {
        return ProtolsConfig::default();
    };

    basic_toml::from_slice(&bytes).unwrap_or_default()
}

#[cfg(test)]
mod test {
    use async_lsp::lsp_types::{Url, WorkspaceFolder};
    use insta::assert_yaml_snapshot;
    use std::path::PathBuf;
    use tempfile::tempdir;

    use super::{CONFIG_FILE_NAMES, WorkspaceProtoConfigs, configure_workspace_folders};

    #[tokio::test(flavor = "current_thread")]
    async fn test_get_for_workspace() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let tmpdir2 = tempdir().expect("failed to create temp2 directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        let folders = vec![
            WorkspaceFolder {
                uri: Url::from_directory_path(tmpdir.path()).unwrap(),
                name: "Test".to_string(),
            },
            WorkspaceFolder {
                uri: Url::from_directory_path(tmpdir2.path()).unwrap(),
                name: "Test2".to_string(),
            },
        ];

        let configured_data = configure_workspace_folders(&folders).await;

        assert_eq!(configured_data.len(), folders.len());

        let mut ws = WorkspaceProtoConfigs::new(vec![], None);

        for data in configured_data {
            ws.add_workspace_folder(data);
        }

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

    #[tokio::test(flavor = "current_thread")]
    async fn test_get_formatter_for_uri() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let tmpdir2 = tempdir().expect("failed to create temp2 directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        let folders = vec![
            WorkspaceFolder {
                uri: Url::from_directory_path(tmpdir.path()).unwrap(),
                name: "Test".to_string(),
            },
            WorkspaceFolder {
                uri: Url::from_directory_path(tmpdir2.path()).unwrap(),
                name: "Test2".to_string(),
            },
        ];

        let configured_data = configure_workspace_folders(&folders).await;

        assert_eq!(configured_data.len(), folders.len());

        let mut ws = WorkspaceProtoConfigs::new(vec![], None);

        for data in configured_data {
            ws.add_workspace_folder(data);
        }

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

    #[tokio::test(flavor = "current_thread")]
    async fn test_loading_different_config_files() {
        let tmpdir = tempdir().expect("failed to create temp directory");

        for file in CONFIG_FILE_NAMES {
            let f = tmpdir.path().join(file);
            std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

            let folders = vec![WorkspaceFolder {
                uri: Url::from_directory_path(tmpdir.path()).unwrap(),
                name: "Test".to_string(),
            }];

            let configured_data = configure_workspace_folders(&folders).await;

            assert_eq!(configured_data.len(), folders.len());

            let mut ws = WorkspaceProtoConfigs::new(vec![], None);
            for data in configured_data {
                ws.add_workspace_folder(data);
            }

            // check we really loaded the config file
            let workspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
            assert!(ws.get_workspace_for_uri(&workspace).is_some());
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_cli_include_paths() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        let absolute_path = tmpdir.path().join("absolute_test_path");

        // Set CLI include paths
        let cli_paths = vec![absolute_path.clone(), PathBuf::from("relative/path")];

        let folders = vec![WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir.path()).unwrap(),
            name: "Test".to_string(),
        }];

        let configured_data = configure_workspace_folders(&folders).await;

        assert_eq!(configured_data.len(), folders.len());

        let mut ws = WorkspaceProtoConfigs::new(cli_paths, None);
        for data in configured_data {
            ws.add_workspace_folder(data);
        }

        let inworkspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
        let include_paths = ws.get_include_paths(&inworkspace).unwrap();

        // The absolute path should be included as is
        assert!(include_paths.contains(&absolute_path));

        // The relative path should be resolved relative to the workspace
        let resolved_relative_path = tmpdir.path().join("relative/path");
        assert!(include_paths.contains(&resolved_relative_path));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_init_include_paths() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        let cli_absolute_path = tmpdir.path().join("cli/path");
        let init_absolute_path = tmpdir.path().join("init/path1");

        // Set both CLI and initialization include paths
        let cli_paths = vec![cli_absolute_path.clone()];
        let init_paths = vec![
            init_absolute_path.clone(),
            PathBuf::from("relative/init/path"),
        ];

        let folders = vec![WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir.path()).unwrap(),
            name: "Test".to_string(),
        }];

        let configured_data = configure_workspace_folders(&folders).await;

        assert_eq!(configured_data.len(), folders.len());

        let mut ws = WorkspaceProtoConfigs::new(cli_paths, None);
        ws.set_init_include_paths(init_paths);
        for data in configured_data {
            ws.add_workspace_folder(data);
        }

        let inworkspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
        let include_paths = ws.get_include_paths(&inworkspace).unwrap();

        // Check that initialization paths are included
        assert!(include_paths.contains(&init_absolute_path));

        // The relative path should be resolved relative to the workspace
        let resolved_relative_path = tmpdir.path().join("relative/init/path");
        assert!(include_paths.contains(&resolved_relative_path));

        // CLI paths should still be included
        assert!(include_paths.contains(&cli_absolute_path));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_fallback_include_path() {
        let tmpdir = tempdir().expect("failed to create temp directory");
        let f = tmpdir.path().join("protols.toml");
        std::fs::write(f, include_str!("input/protols-valid.toml")).unwrap();

        // Set both CLI and initialization include paths
        let cli_paths = vec![PathBuf::from("/cli/path")];
        let init_paths = vec![
            PathBuf::from("/init/path1"),
            PathBuf::from("relative/init/path"),
        ];

        let folders = vec![WorkspaceFolder {
            uri: Url::from_directory_path(tmpdir.path()).unwrap(),
            name: "Test".to_string(),
        }];

        let configured_data = configure_workspace_folders(&folders).await;

        assert_eq!(configured_data.len(), folders.len());

        let mut ws = WorkspaceProtoConfigs::new(cli_paths, Some("fallback_path".into()));
        ws.set_init_include_paths(init_paths);
        for data in configured_data {
            ws.add_workspace_folder(data);
        }

        let inworkspace = Url::from_file_path(tmpdir.path().join("foobar.proto")).unwrap();
        let include_paths = ws.get_include_paths(&inworkspace).unwrap();

        // Fallback path should be included and on the last position
        assert_eq!(
            include_paths
                .iter()
                .rev()
                .position(|p| p == "fallback_path"),
            Some(0)
        );
    }
}
