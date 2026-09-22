use serde::Deserialize;
use std::{collections::HashMap, env, fs};
use zed_extension_api::serde_json::json;
use zed_extension_api::settings::LspSettings;
use zed_extension_api::{self as zed, Result, serde_json};

const SERVER_PATH: &str = "node_modules/@mdx-js/language-server/lib/index.js";
const PACKAGE_NAME: &str = "@mdx-js/language-server";

const TYPESCRIPT_PACKAGE_NAME: &str = "typescript";
// The language server loads TypeScript's JavaScript API, which TypeScript 7+
// no longer ships, so the tsdk is pinned to a 6.x release.
const TYPESCRIPT_VERSION: &str = "6.0.3";
const TS_PLUGIN_PACKAGE_NAME: &str = "@mdx-js/typescript-plugin";

const TYPESCRIPT_TSDK_PATH: &str = "node_modules/typescript/lib";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageJson {
    #[serde(default)]
    dependencies: HashMap<String, String>,
    #[serde(default)]
    dev_dependencies: HashMap<String, String>,
}

struct MDXExtension {
    did_find_server: bool,
    typescript_sdk_path: String,
}

impl MDXExtension {
    fn server_exists(&self) -> bool {
        fs::metadata(SERVER_PATH).map_or(false, |stat| stat.is_file())
    }

    fn server_script_path(&mut self, language_server_id: &zed::LanguageServerId) -> Result<String> {
        let server_exists = self.server_exists();
        if self.did_find_server && server_exists {
            self.install_typescript_if_needed()?;
            self.install_ts_plugin_if_needed()?;
            return Ok(SERVER_PATH.to_string());
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let version = zed::npm_package_latest_version(PACKAGE_NAME)?;

        if !server_exists
            || zed::npm_package_installed_version(PACKAGE_NAME)?.as_ref() != Some(&version)
        {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );
            let latest_version = zed::npm_package_latest_version(PACKAGE_NAME)?;
            let result = zed::npm_install_package(PACKAGE_NAME, &latest_version);

            match result {
                Ok(()) => {
                    if !self.server_exists() {
                        Err(format!(
                            "installed package '{PACKAGE_NAME}' did not contain expected path '{SERVER_PATH}'",
                        ))?;
                    }
                }
                Err(error) => {
                    if !self.server_exists() {
                        Err(error)?;
                    }
                }
            }
        }

        self.install_typescript_if_needed()?;
        self.did_find_server = true;
        Ok(SERVER_PATH.to_string())
    }

    /// Installs the TypeScript used as the tsdk, instead of the worktree's.
    fn install_typescript_if_needed(&mut self) -> Result<()> {
        let installed_typescript_version =
            zed::npm_package_installed_version(TYPESCRIPT_PACKAGE_NAME)?;

        if installed_typescript_version.as_deref() != Some(TYPESCRIPT_VERSION) {
            println!("installing {TYPESCRIPT_PACKAGE_NAME}@{TYPESCRIPT_VERSION}");
            zed::npm_install_package(TYPESCRIPT_PACKAGE_NAME, TYPESCRIPT_VERSION)?;
        } else {
            println!("typescript already installed");
        }

        self.typescript_sdk_path = env::current_dir()
            .unwrap()
            .join(TYPESCRIPT_TSDK_PATH)
            .to_string_lossy()
            .to_string();

        Ok(())
    }

    fn install_ts_plugin_if_needed(&mut self) -> Result<()> {
        let installed_plugin_version = zed::npm_package_installed_version(TS_PLUGIN_PACKAGE_NAME)?;
        let latest_plugin_version = zed::npm_package_latest_version(TS_PLUGIN_PACKAGE_NAME)?;

        if installed_plugin_version.as_ref() != Some(&latest_plugin_version) {
            println!("installing {TS_PLUGIN_PACKAGE_NAME}@{latest_plugin_version}");
            zed::npm_install_package(TS_PLUGIN_PACKAGE_NAME, &latest_plugin_version)?;
        } else {
            println!("ts-plugin already installed");
        }
        Ok(())
    }

    fn get_ts_plugin_root_path(&self, worktree: &zed::Worktree) -> Result<Option<String>> {
        let package_json = worktree.read_text_file("package.json")?;
        let package_json: PackageJson = serde_json::from_str(&package_json)
            .map_err(|err| format!("failed to parse package.json: {err}"))?;

        let has_local_plugin = package_json
            .dev_dependencies
            .contains_key(TS_PLUGIN_PACKAGE_NAME)
            || package_json
                .dependencies
                .contains_key(TS_PLUGIN_PACKAGE_NAME);

        if has_local_plugin {
            println!("Using local installation of {TS_PLUGIN_PACKAGE_NAME}");
            return Ok(None);
        }

        println!("Using global installation of {TS_PLUGIN_PACKAGE_NAME}");
        Ok(Some(
            env::current_dir().unwrap().to_string_lossy().to_string(),
        ))
    }
}

impl zed::Extension for MDXExtension {
    fn new() -> Self {
        Self {
            did_find_server: false,
            typescript_sdk_path: TYPESCRIPT_TSDK_PATH.to_owned(),
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let server_path = self.server_script_path(language_server_id)?;
        Ok(zed::Command {
            command: zed::node_binary_path()?,
            args: vec![
                env::current_dir()
                    .unwrap()
                    .join(&server_path)
                    .to_string_lossy()
                    .to_string(),
                "--stdio".to_string(),
            ],
            env: Default::default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        let initialization_options = LspSettings::for_worktree("mdx", worktree)
            .ok()
            .and_then(|settings| settings.initialization_options)
            .unwrap_or_else(|| {
                json!({
                    "typescript": {
                        "tsdk": self.typescript_sdk_path
                    },
                })
            });
        println!(
            "MDX Analyzer initialization options {:?}",
            initialization_options
        );

        Ok(Some(initialization_options))
    }

    fn language_server_additional_initialization_options(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        target_language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        match target_language_server_id.as_ref() {
            "typescript-language-server" => Ok(Some(serde_json::json!({
                "plugins": [{
                    "name": "@mdx-js/typescript-plugin",
                    "location": self.get_ts_plugin_root_path(worktree)?.unwrap_or_else(|| worktree.root_path()),
                    "configNamespace": "typescript",
                    "languages": ["mdx"],
                }],
            }))),
            _ => Ok(None),
        }
    }

    fn language_server_additional_workspace_configuration(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        target_language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        match target_language_server_id.as_ref() {
            "vtsls" => Ok(Some(serde_json::json!({
                "vtsls": {
                    "tsserver": {
                        "globalPlugins": [{
                            "name": "@mdx-js/typescript-plugin",
                            "location": self.get_ts_plugin_root_path(worktree)?.unwrap_or_else(|| worktree.root_path()),
                            "enableForWorkspaceTypeScriptVersions": true,
                            "configNamespace": "typescript",
                            "languages": ["mdx"],
                        }]
                    }
                },
            }))),
            _ => Ok(None),
        }
    }
}

zed::register_extension!(MDXExtension);
