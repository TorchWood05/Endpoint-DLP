use super::schema::{ArgusConfig, CONFIG_DIR, CONFIG_FILE};
use crate::fsutil::{check_dir_security, read_trusted_to_string};
use anyhow::{Context, Result};
use std::io::ErrorKind;
use std::path::Path;
use tracing::{info, warn};

const MAX_CONFIG_BYTES: u64 = 64 * 1024;

pub fn load_config() -> Result<ArgusConfig> {
    load_config_from(Path::new(CONFIG_DIR))
}

fn load_config_from(dir: &Path) -> Result<ArgusConfig> {
    let path = dir.join(CONFIG_FILE);
    let text = check_dir_security(dir).and_then(|_| read_trusted_to_string(&path, MAX_CONFIG_BYTES));

    let cfg = match text {
        Ok(text) => {
            info!("loading system config from {}", path.display());
            toml::from_str::<ArgusConfig>(&text)
                .with_context(|| format!("invalid config file {}", path.display()))?
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            warn!(
                "system config {} not found, using built-in defaults",
                path.display()
            );
            ArgusConfig::default()
        }
        Err(e) => {
            return Err(e).with_context(|| format!("cannot use config {}", path.display()));
        }
    };

    cfg.validate()?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::RunMode;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn write_config(dir: &Path, body: &str, mode: u32) {
        let p = dir.join(CONFIG_FILE);
        fs::write(&p, body).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
    }

    fn secure(dir: &Path) {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn body_for(watch: &Path) -> String {
        format!(
            "[agent]\nrun_mode = \"enforce\"\n[sensors.fanotify]\nwatch_paths = [{:?}]\n",
            watch.display().to_string()
        )
    }

    #[test]
    fn loads_valid_config() {
        let d = tempdir().unwrap();
        secure(d.path());
        write_config(d.path(), &body_for(d.path()), 0o600);
        let c = load_config_from(d.path()).unwrap();
        assert_eq!(c.agent.run_mode, RunMode::Enforce);
    }

    #[test]
    fn malformed_config_is_fatal() {
        let d = tempdir().unwrap();
        secure(d.path());
        write_config(d.path(), "this is = not [valid", 0o600);
        assert!(load_config_from(d.path()).is_err());
    }

    #[test]
    fn writable_config_is_rejected() {
        let d = tempdir().unwrap();
        secure(d.path());
        write_config(d.path(), &body_for(d.path()), 0o666);
        assert!(load_config_from(d.path()).is_err());
    }

    #[test]
    fn loose_directory_is_rejected() {
        let d = tempdir().unwrap();
        write_config(d.path(), &body_for(d.path()), 0o600);
        fs::set_permissions(d.path(), fs::Permissions::from_mode(0o777)).unwrap();
        assert!(load_config_from(d.path()).is_err());
    }

    #[test]
    fn missing_directory_falls_back_to_defaults() {
        if !Path::new("/home").is_dir() {
            return;
        }
        let d = tempdir().unwrap();
        let c = load_config_from(&d.path().join("absent")).unwrap();
        assert_eq!(c, ArgusConfig::default());
    }
}