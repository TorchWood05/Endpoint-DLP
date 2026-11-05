use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use tracing::info;

use super::schema::{CONFIG_DIR, CONFIG_FILE};

const DEFAULT_CONFIG: &str = include_str!("../../configs/config.toml");

pub fn install_default_config() -> Result<()> {
    let config_dir = Path::new(CONFIG_DIR);

    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(config_dir)
        .with_context(|| {
            format!(
                "Failed to create Argus config directory {}",
                config_dir.display()
            )
        })?;

    let config_path = config_dir.join(CONFIG_FILE);

    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&config_path)
    {
        Ok(file) => file,

        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            info!(
                "Argus configuration already exists at {}",
                config_path.display()
            );
            return Ok(());
        }

        Err(e) => {
            return Err(e).with_context(|| {
                format!(
                    "Failed to create default configuration {}",
                    config_path.display()
                )
            });
        }
    };

    if let Err(e) = file
        .write_all(DEFAULT_CONFIG.as_bytes())
        .and_then(|_| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(&config_path);

        return Err(e).with_context(|| {
            format!(
                "Failed to write default configuration {}",
                config_path.display()
            )
        });
    }

    info!(
        "Installed default Argus configuration at {}",
        config_path.display()
    );

    Ok(())
}