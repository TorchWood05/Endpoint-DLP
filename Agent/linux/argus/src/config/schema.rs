use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::PathBuf;

pub const CONFIG_DIR: &str = "/etc/argus/config";
pub const CONFIG_FILE: &str = "config.toml";
pub const DATA_DIR: &str = "/var/lib/argus";
pub const ID_FILE: &str = "/var/lib/argus/argusId";

#[allow(dead_code)]
pub const MANAGED_DIR: &str = "/var/lib/argus/managed";
#[allow(dead_code)]
pub const STATE_DIR: &str = "/var/lib/argus/state";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArgusConfig {
    pub agent: AgentConfig,
    pub logging: LoggingConfig,
    pub sensors: SensorsConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    #[default]
    Monitor,
    Enforce,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AgentConfig {
    pub run_mode: RunMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoggingConfig {
    pub level: LogLevel,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SensorsConfig {
    pub fanotify: FanotifyConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FanotifyConfig {
    pub enabled: bool,
    pub watch_paths: Vec<PathBuf>,
}

impl Default for FanotifyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            watch_paths: vec![PathBuf::from("/home")],
        }
    }
}

impl ArgusConfig {
    pub fn validate(&self) -> Result<()> {
        let f = &self.sensors.fanotify;
        if f.enabled {
            if f.watch_paths.is_empty() {
                bail!("config: sensors.fanotify.watch_paths must not be empty");
            }
            for p in &f.watch_paths {
                if !p.is_absolute() || !p.is_dir() {
                    bail!(
                        "config: watch path {} must be an absolute, existing directory",
                        p.display()
                    );
                }
            }
        }
        Ok(())
    }
}