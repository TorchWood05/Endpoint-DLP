use super::schema::{ArgusConfig, CONFIG_DIR, CONFIG_FILE};
use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use tracing::{info, warn};

pub fn load_config() -> Result<ArgusConfig> {
    let path = Path::new(CONFIG_DIR).join(CONFIG_FILE);

    let cfg = match fs::read_to_string(&path) {
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
            return Err(e).with_context(|| format!("cannot read {}", path.display()));
        }
    };

    cfg.validate()?;
    Ok(cfg)
}