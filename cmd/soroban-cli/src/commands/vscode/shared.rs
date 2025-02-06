use std::path::{Path, PathBuf};

use directories::BaseDirs;
use jsonc_parser::{
    cst::{CstNode, CstRootNode},
    json, ParseOptions,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("⛔ ️Failed to VsCode Settings")]
    FailedToFind,
    #[error("⛔ ️Settings does not exist: {0}")]
    SettingsDoNotExist(String),
    #[error("Failed to read Settings {0:?}")]
    ReadSettings(PathBuf),
    #[error("Failed to write Settings {0:?}")]
    WriteSettings(PathBuf),
    #[error("Failed to parse JSON {0:?}\n{1}")]
    FailedToParseJSON(PathBuf, #[source] serde_json::Error),
    #[error(transparent)]
    JsonC(#[from] jsonc_parser::errors::ParseError),
    #[error("json.schemas is not an array")]
    JsonSchemasNotAnArray,
}

fn get_vscode_settings_path() -> Option<PathBuf> {
    if let Some(base_dirs) = BaseDirs::new() {
        #[cfg(target_os = "windows")]
        let settings_path = base_dirs
            .config_dir()
            .join("Code")
            .join("User")
            .join("settings.json");

        #[cfg(target_os = "macos")]
        let settings_path = base_dirs
            .config_dir()
            .join("Code")
            .join("User")
            .join("settings.json");

        #[cfg(target_os = "linux")]
        let settings_path = base_dirs
            .config_dir()
            .join("Code")
            .join("User")
            .join("settings.json");

        Some(settings_path)
    } else {
        None
    }
}
#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// Path to settings file. Default is global install
    #[arg(long)]
    settings: Option<PathBuf>,
}

impl Args {
    pub fn settings(&self) -> Result<PathBuf, Error> {
        find_vscode_settings(self.settings.as_deref())
    }

    pub fn settings_dir(&self) -> Result<PathBuf, Error> {
        self.settings().map(|p| p.parent().unwrap().to_path_buf())
    }

    pub fn read_settings(&self) -> Result<VsCodeSettings, Error> {
        let settings_path = self.settings()?;
        tracing::debug!("Reading settings from {:?}", settings_path);
        let settings = std::fs::read_to_string(&settings_path)
            .map_err(|_| Error::ReadSettings(settings_path.clone()))?;
        settings.parse()
    }

    pub fn write_settings(&self, settings: &VsCodeSettings) -> Result<(), Error> {
        let settings_path = self.settings()?;
        let settings = settings.to_string();
        tracing::debug!("Writing settings {settings} to {settings_path:?}");
        std::fs::write(&settings_path, settings)
            .map_err(|_| Error::WriteSettings(settings_path))?;
        Ok(())
    }

    pub fn write_schema_file(&self) -> Result<(), Error> {
        let p = self.settings_dir()?.join("transaction_env.json");
        tracing::debug!("Writing transaction envelope schema to {p:?}");
        std::fs::write(&p, TXN_ENV_SCHEMA).map_err(|_| Error::WriteSettings(p))?;
        Ok(())
    }
}

pub fn find_vscode_settings(settings: Option<&Path>) -> Result<PathBuf, Error> {
    let settings = settings
        .map_or_else(get_vscode_settings_path, |p| Some(p.to_path_buf()))
        .ok_or(Error::FailedToFind)?;
    if settings.exists() {
        Ok(settings)
    } else {
        Err(Error::SettingsDoNotExist(
            settings.to_string_lossy().to_string(),
        ))
    }
}

/// Becasue the VS Code settings use a variant of json with comments, `json-parser` crate must be used to parse
/// and update it to preserve comments.
#[derive(Debug, Clone)]
pub struct VsCodeSettings(jsonc_parser::cst::CstRootNode);

impl std::str::FromStr for VsCodeSettings {
    type Err = Error;

    fn from_str(content: &str) -> Result<Self, Self::Err> {
        Ok(Self(CstRootNode::parse(content, &ParseOptions::default())?))
    }
}

impl std::fmt::Display for VsCodeSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)?;
        Ok(())
    }
}

impl VsCodeSettings {
    pub fn add_schema(&mut self, base_dir: &Path) -> Result<bool, Error> {
        let settings = self.0.object_value_or_set();
        let Some(schemas) = settings.array_value_or_create("json.schemas") else {
            return Err(Error::JsonSchemasNotAnArray);
        };
        if schemas.elements().iter().any(is_stellar_schema) {
            return Ok(false);
        }
        let url = base_dir
            .join("transaction_env.json")
            .to_string_lossy()
            .to_string();
        let input = json!( {
            "stellar": true,
            "fileMatch": ["**/*.stellar_txn.json"],
            "url": url,
        });
        schemas.append(input);
        Ok(true)
    }
}

fn is_stellar_schema(s: &CstNode) -> bool {
    s.as_object()
        .and_then(|o| o.get("stellar"))
        .and_then(|v| v.value())
        .and_then(|v| v.as_boolean_lit())
        .is_some_and(|b| b.value())
}

const TXN_ENV_SCHEMA: &str = include_str!("../../fixtures/transaction_env.json");

#[cfg(test)]
mod test {
    const SETTINGS: &str = r#"
{
    "terminal.integrated.scrollback": 10000,
    "tabnine.experimentalAutoImports": true,
    "editor.codeActionsOnSave": {},
    "rust-analyzer.cargo.extraEnv": {
        "PROTOC": "/opt/homebrew/bin/protoc",
        "DYLD_FALLBACK_LIBRARY_PATH":"/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/"
        // "RUSTFLAGS": "-Dwarnings"
    },
    "continue.telemetryEnabled": false,
    "workbench.sideBar.location": "right",
    "[html]": {
        "editor.defaultFormatter": "rvest.vs-code-prettier-eslint"
    }
}
"#;
    fn settings_after(s: &str) -> String {
        format!(
            r#"
{{
    "terminal.integrated.scrollback": 10000,
    "tabnine.experimentalAutoImports": true,
    "editor.codeActionsOnSave": {{}},
    "rust-analyzer.cargo.extraEnv": {{
        "PROTOC": "/opt/homebrew/bin/protoc",
        "DYLD_FALLBACK_LIBRARY_PATH":"/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/"
        // "RUSTFLAGS": "-Dwarnings"
    }},
    "continue.telemetryEnabled": false,
    "workbench.sideBar.location": "right",
    "[html]": {{
        "editor.defaultFormatter": "rvest.vs-code-prettier-eslint"
    }},
    "json.schemas": [
        {{
            "stellar": true,
            "fileMatch": ["**/*.stellar_txn.json"],
            "url": "{s}/transaction_env.json"
        }}
    ]
}}
"#
        )
    }

    #[test]
    fn test_add_schema() {
        let mut settings: super::VsCodeSettings = SETTINGS.parse().unwrap();
        let base_dir = std::env::current_dir().unwrap();
        let added = settings.add_schema(&base_dir).unwrap();
        assert!(added);
        let after: String = settings_after(&base_dir.to_string_lossy());
        assert_eq!(after, settings.to_string());
        let added = settings.add_schema(&base_dir).unwrap();
        assert!(!added);
        assert_eq!(after, settings.to_string());
    }
}
