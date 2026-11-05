use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::PathBuf;

pub const CONFIG_DIR: &str = "/etc/argus/config";
pub const CONFIG_FILE: &str = "config.toml";
pub const DATA_DIR: &str = "/var/lib/argus";
pub const ID_FILE_NAME: &str = "argusId";

#[allow(dead_code)]
pub const MANAGED_DIR: &str = "/var/lib/argus/managed";
#[allow(dead_code)]
pub const STATE_DIR: &str = "/var/lib/argus/state";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LoggingConfig {
    pub level: LogLevel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SensorsConfig {
    pub fanotify: FanotifyConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn cfg_with_paths(paths: Vec<PathBuf>) -> ArgusConfig {
        let mut c = ArgusConfig::default();
        c.sensors.fanotify.watch_paths = paths;
        c
    }

    #[test]
    fn defaults_are_monitor_info_fanotify_home() {
        let c = ArgusConfig::default();
        assert_eq!(c.agent.run_mode, RunMode::Monitor);
        assert_eq!(c.logging.level, LogLevel::Info);
        assert!(c.sensors.fanotify.enabled);
        assert_eq!(c.sensors.fanotify.watch_paths, vec![PathBuf::from("/home")]);
    }

    #[test]
    fn partial_config_keeps_other_defaults() {
        let c: ArgusConfig = toml::from_str("[agent]\nrun_mode = \"enforce\"").unwrap();
        assert_eq!(c.agent.run_mode, RunMode::Enforce);
        assert_eq!(c.logging, LoggingConfig::default());
        assert_eq!(c.sensors, SensorsConfig::default());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<ArgusConfig>("[agent]\nrun_mod = \"enforce\"").is_err());
    }

    #[test]
    fn validate_accepts_existing_absolute_dir() {
        let d = tempdir().unwrap();
        assert!(cfg_with_paths(vec![d.path().to_path_buf()]).validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_paths_when_enabled() {
        assert!(cfg_with_paths(vec![]).validate().is_err());
    }

    #[test]
    fn validate_rejects_relative_and_missing_paths() {
        assert!(cfg_with_paths(vec![PathBuf::from("relative/dir")]).validate().is_err());
        let d = tempdir().unwrap();
        assert!(cfg_with_paths(vec![d.path().join("missing")]).validate().is_err());
    }

    #[test]
    fn validate_ignores_paths_when_fanotify_disabled() {
        let mut c = cfg_with_paths(vec![]);
        c.sensors.fanotify.enabled = false;
        assert!(c.validate().is_ok());
    }
}