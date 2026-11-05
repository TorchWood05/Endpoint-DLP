mod config;

use anyhow::{Context, Result};
use nix::sys::utsname::uname;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use tracing::info;
use uuid::Uuid;

#[derive(Debug)]
#[allow(dead_code)]
struct SystemInfo {
    os: String,
    kernel_version: String,
    distro_id: String,
    architecture: String,
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
        .context("Failed to determine distro_id")?;

    let distro_id = os_release
        .lines()
        .find_map(|line| line.strip_prefix("ID="))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| "Unknown".to_string());

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

fn agent_id() -> Result<String> {
    let dir = Path::new(config::DATA_DIR);
    let path = Path::new(config::ID_FILE);

    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .with_context(|| format!("Failed to create data directory {}", dir.display()))?;

    match fs::read_to_string(path) {
        Ok(saved) => {
            let id = Uuid::parse_str(saved.trim())
                .context("Existing Argus ID is invalid / corrupted")?;
            info!("Argus ID loaded");
            return Ok(id.to_string());
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e).context("Failed to read Argus ID"),
    }

    let id = Uuid::new_v4();
    let tmp = dir.join("argusId.tmp");

    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(e).context("Failed to remove stale temp file"),
    }

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(&tmp)
        .context("Failed to create Argus ID temp file")?;

    writeln!(file, "{id}").context("Failed to write Argus ID")?;
    file.sync_all().context("Failed to persist Argus ID")?;
    drop(file);

    fs::rename(&tmp, path).context("Failed to save Argus ID")?;

    File::open(dir)
        .and_then(|d| d.sync_all())
        .context("Failed to sync data directory")?;

    info!("New Argus ID created");
    Ok(id.to_string())
}

fn init() -> Result<()> {
    info!("Initializing Argus");
    info!("Checking Argus configuration");
    config::install_default_config().context("Failed to setup default configuration")?;
    info!("Loading configuration");
    let _cfg = config::load_config()?;
    info!("Configuration loaded");
    let _id = agent_id()?;
    info!("Collecting system information");
    let _sysinfo = system_info()?;
    info!("Argus initialized");
    Ok(())
}

fn main() {
    tracing_subscriber::fmt::init();

    if let Err(e) = init() {
        tracing::error!("fatal: {e:#}");
        std::process::exit(1);
    }
}