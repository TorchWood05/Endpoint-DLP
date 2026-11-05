mod config;
mod fsutil;

use anyhow::{Context, Result};
use config::{LogLevel, DATA_DIR, ID_FILE_NAME};
use fsutil::{create_file_atomically, ensure_dir_security, read_trusted_to_string};
use nix::sys::utsname::uname;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use tracing::{info, warn};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{reload, Registry};
use uuid::Uuid;

type LogHandle = reload::Handle<LevelFilter, Registry>;

#[derive(Debug)]
#[allow(dead_code)] 
struct SystemInfo {
    os: String,
    kernel_version: String,
    distro_id: String,
    architecture: String,
}

fn parse_distro_id(os_release: &str) -> String {
    os_release
        .lines()
        .find_map(|line| line.strip_prefix("ID="))
        .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn system_info() -> Result<SystemInfo> {
    let uname = uname().context("Failed to get system information (uname)")?;

    let kernel_release = uname.release().to_string_lossy().into_owned();
    let kernel_version = kernel_release
        .split(['-', '+'])
        .next()
        .unwrap_or(&kernel_release)
        .to_string();

    let os = uname.sysname().to_string_lossy().into_owned();
    let architecture = uname.machine().to_string_lossy().into_owned();

    let os_release = fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    let distro_id = parse_distro_id(&os_release);

    info!("OS             : {os}");
    info!("Distro ID      : {distro_id}");
    info!("Kernel Version : {kernel_version}");
    info!("Architecture   : {architecture}");

    Ok(SystemInfo {
        os,
        kernel_version,
        distro_id,
        architecture,
    })
}

fn read_id(path: &Path) -> Result<Option<Uuid>> {
    match read_trusted_to_string(path, 64) {
        Ok(text) => {
            let id = Uuid::parse_str(text.trim())
                .context("Existing Argus ID is invalid / corrupted")?;
            Ok(Some(id))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("Failed to read Argus ID"),
    }
}

fn load_or_create_agent_id(dir: &Path) -> Result<Uuid> {
    ensure_dir_security(dir)
        .with_context(|| format!("Failed to prepare data directory {}", dir.display()))?;

    let path = dir.join(ID_FILE_NAME);

    if let Some(id) = read_id(&path)? {
        info!("Argus ID loaded");
        return Ok(id);
    }

    let id = Uuid::new_v4();
    match create_file_atomically(dir, ID_FILE_NAME, format!("{id}\n").as_bytes(), 0o400) {
        Ok(()) => {
            info!("New Argus ID created");
            Ok(id)
        }
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            let existing = read_id(&path)?.context("Argus ID disappeared after creation race")?;
            info!("Argus ID loaded");
            Ok(existing)
        }
        Err(e) => Err(e).context("Failed to save Argus ID"),
    }
}

fn agent_id() -> Result<Uuid> {
    load_or_create_agent_id(Path::new(DATA_DIR))
}

fn level_filter(level: LogLevel) -> LevelFilter {
    match level {
        LogLevel::Error => LevelFilter::ERROR,
        LogLevel::Warn => LevelFilter::WARN,
        LogLevel::Info => LevelFilter::INFO,
        LogLevel::Debug => LevelFilter::DEBUG,
    }
}

fn init_logging() -> LogHandle {
    let (filter, handle) = reload::Layer::new(LevelFilter::INFO);
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
    handle
}

fn init(log: &LogHandle) -> Result<()> {
    info!("Initializing Argus");
    info!("Checking Argus configuration");
    if let Err(e) = config::install_default_config() {
        warn!("could not install default configuration, continuing: {e:#}");
    }

    info!("Loading configuration");
    let cfg = config::load_config()?;

    log.modify(|f| *f = level_filter(cfg.logging.level))
        .context("Failed to apply logging level")?;
    info!(
        "Configuration loaded"
    );

    let _id = agent_id()?;
    info!("Argus identity initialized");

    info!("Collecting system information");
    let _sysinfo = system_info()?;

    info!("Argus initialized");
    Ok(())
}

fn main() {
    let log = init_logging();

    if let Err(e) = init(&log) {
        tracing::error!("fatal: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn creates_then_reloads_same_id() {
        let d = tempdir().unwrap();
        let dir = d.path().join("data");
        let first = load_or_create_agent_id(&dir).unwrap();
        let second = load_or_create_agent_id(&dir).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn corrupt_id_is_an_error_not_regenerated() {
        let d = tempdir().unwrap();
        let dir = d.path().join("data");
        ensure_dir_security(&dir).unwrap();
        let p = dir.join(ID_FILE_NAME);
        fs::write(&p, "not-a-uuid").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(load_or_create_agent_id(&dir).is_err());
    }

    #[test]
    fn loose_data_dir_is_rejected() {
        let d = tempdir().unwrap();
        let dir = d.path().join("data");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(load_or_create_agent_id(&dir).is_err());
    }

    #[test]
    fn distro_id_parsing() {
        assert_eq!(parse_distro_id("NAME=x\nID=ubuntu\nID_LIKE=debian"), "ubuntu");
        assert_eq!(parse_distro_id("ID=\"fedora\""), "fedora");
        assert_eq!(parse_distro_id("ID='arch'"), "arch");
        assert_eq!(parse_distro_id("ID_LIKE=debian"), "unknown");
        assert_eq!(parse_distro_id(""), "unknown");
    }
}