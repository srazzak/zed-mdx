use std::{fs, collections::HashSet, env};
use zed_extension_api::{self as zed, settings::LspSettings};

struct MDXExtension {
    installed: HashSet<String>,
}

const PACKAGE_NAME: &str = "@mdx-js/language-server";
const SERVER_PATH: &str = "node_modules/.bin/mdx-language-server";
const SERVER_NAME: &str = "language-server";

impl MDXExtension {
    fn server_exists(&self) -> bool {
        fs::metadata(SERVER_PATH).is_ok_and(|m| m.is_file())
    }

    fn server_script_path(
        &mut self,
        language_server_id: &zed_extension_api::LanguageServerId,
    ) -> zed::Result<String> {
        if !self.server_exists() {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::CheckingForUpdate,
            );
            let version = zed::npm_package_latest_version(PACKAGE_NAME)?;

            if zed::npm_package_installed_version(PACKAGE_NAME)?.as_ref() != Some(&version) {
                zed::set_language_server_installation_status(
                    language_server_id,
                    &zed::LanguageServerInstallationStatus::Downloading,
                );
                let result = zed::npm_install_package(PACKAGE_NAME, &version);
                if !self.server_exists() {
                    return result.and_then(|_| Err(format!("installed package '{PACKAGE_NAME}' did not contain expected path '{SERVER_PATH}'")));
                }
            }
        }

        Ok(SERVER_PATH.to_string())
    }
}

impl zed::Extension for MDXExtension {
    fn new() -> Self {
        Self {
            installed: HashSet::new(),
        }
    }


    fn language_server_initialization_options(
        &mut self,
        _: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        let init_options = LspSettings::for_worktree(SERVER_NAME, worktree)
            .ok()
            .and_then(|settings| settings.initialization_options)
            .and_then(|options| options.get("typescript").cloned())
            .and_then(|options| options.as_object().cloned());

        let cwd = env::current_dir();
        let zed_cwd = cwd.unwrap().ancestors().nth(3).unwrap().join("languages/vtsls/node_modules/typescript/lib").to_string_lossy().to_string();

        let ts_enabled = init_options
            .as_ref()
            .and_then(|options| options.get("enabled").and_then(|enabled| enabled.as_bool()))
            .unwrap_or(true);
        let tsdk_path = init_options
            .as_ref()
            .and_then(|options| {
                options
                    .get("tsdk")
                    .and_then(|tsdk| tsdk.as_str())
                    .map(|s| s.to_owned())
                    .clone()
            })
            .unwrap_or(zed_cwd);

        Ok(Some(zed::serde_json::json!({
            "typescript": {
                "enabled": ts_enabled,
                "tsdk": tsdk_path,
            },
        })))
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        _: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let server_path = self.server_script_path(language_server_id)?;

        Ok(zed::Command {
            command: zed::node_binary_path()?,
            args: vec![
                env::current_dir()
                    .unwrap()
                    .join(&server_path)
                    .to_string_lossy()
                    .to_string(),
                "--stdio".to_owned(),
            ],
            env: Default::default(),
        })
    }
}

zed::register_extension!(MDXExtension);

/// Extensions to the Zed extension API that have not yet stabilized.
mod zed_ext {
    /// Sanitizes the given path to remove the leading `/` on Windows.
    ///
    /// On macOS and Linux this is a no-op.
    ///
    /// This is a workaround for https://github.com/bytecodealliance/wasmtime/issues/10415.
    pub fn sanitize_windows_path(path: std::path::PathBuf) -> std::path::PathBuf {
        use zed_extension_api::{current_platform, Os};

        let (os, _arch) = current_platform();
        match os {
            Os::Mac | Os::Linux => path,
            Os::Windows => path
                .to_string_lossy()
                .to_string()
                .trim_start_matches('/')
                .into(),
        }
    }
}
