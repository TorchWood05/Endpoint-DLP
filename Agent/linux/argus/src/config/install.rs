use super::schema::{CONFIG_DIR, CONFIG_FILE};
use crate::fsutil::{create_file_atomically, ensure_dir_security};
use anyhow::{Context, Result};
use std::io::ErrorKind;
use std::path::Path;
use tracing::info;

const DEFAULT_CONFIG: &str = include_str!("../../configs/config.toml");

pub fn install_default_config() -> Result<()> {
    install_default_config_in(Path::new(CONFIG_DIR))
}

fn install_default_config_in(dir: &Path) -> Result<()> {
    ensure_dir_security(dir).with_context(|| {
        format!("Failed to prepare Argus config directory {}", dir.display())
    })?;
    let config_path = dir.join(CONFIG_FILE);
    match create_file_atomically(dir, CONFIG_FILE, DEFAULT_CONFIG.as_bytes(), 0o600) {
        Ok(()) => {
            info!(
                "Installed default Argus configuration at {}",
                config_path.display()
            );
            Ok(())
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            info!(
                "Argus configuration already exists at {}",
                config_path.display()
            );
            Ok(())
        }
        Err(e) => Err(e).with_context(|| {
            format!(
                "Failed to install default configuration {}",
                config_path.display()
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::ArgusConfig;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use tempfile::tempdir;

    #[test]
    fn embedded_default_matches_builtin_defaults() {
        let parsed: ArgusConfig = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(parsed, ArgusConfig::default());
    }

    #[test]
    fn installs_config_with_0600() {
        let d = tempdir().unwrap();
        let dir = d.path().join("cfg");
        install_default_config_in(&dir).unwrap();
        let p = dir.join(CONFIG_FILE);
        assert_eq!(fs::read_to_string(&p).unwrap(), DEFAULT_CONFIG);
        assert_eq!(fs::metadata(&p).unwrap().mode() & 0o777, 0o600);
    }

    #[test]
    fn does_not_overwrite_existing_config() {
        let d = tempdir().unwrap();
        let dir = d.path().join("cfg");
        install_default_config_in(&dir).unwrap();
        let p = dir.join(CONFIG_FILE);
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&p, "[agent]\nrun_mode = \"enforce\"\n").unwrap();
        install_default_config_in(&dir).unwrap();
        assert!(fs::read_to_string(&p).unwrap().contains("enforce"));
    }

    #[test]
    fn rejects_loose_existing_directory() {
        let d = tempdir().unwrap();
        let dir = d.path().join("cfg");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(install_default_config_in(&dir).is_err());
        assert!(!dir.join(CONFIG_FILE).exists());
    }
}