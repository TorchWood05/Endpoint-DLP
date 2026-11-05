#![allow(warnings)]

mod config;

use std::fs;
use std::fs::OpenOptions;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::io::Write;
use std::path::Path;
use uuid::Uuid;
use tracing::{info, warn};
use nix::sys::utsname::uname;
use anyhow::{Context, Result};


#[derive(Debug)]
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
        .context("failed to read os-release")?;

    let distro_id = os_release
        .lines()
        .find_map(|line| line.strip_prefix("ID="))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| "unknown".to_string());

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
    let dir = Path::new("/var/lib/argus");
    let path = dir.join("argusId");

    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .context("Failed to create directory")?;

    match fs::read_to_string(&path) {
        Ok(saved) => {
            let id = Uuid::parse_str(saved.trim())
                .context("Existing Argus ID is either invalid / corrupted")?;

            info!("Argus ID loaded");

            return Ok(id.to_string());
        }

        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound => {}

        Err(error) => {
            return Err(error)
                .context("Failed to read Argus ID");
        }
    }

    let id = Uuid::new_v4();

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .context("Failed to create Argus ID")?;

    writeln!(file, "{id}")
        .context("Failed to write Argus ID")?;

    file.sync_all()
        .context("Failed to persist Argus ID")?;

    info!("New Argus ID created");

    Ok(id.to_string())
}

fn init() -> Result<()> {
    info!("Initializing Argus");
    info!("Loading configuration");
    let configs = String::new();
    let id = agent_id()?;
    info!("Collecting system information");
    let system_info = system_info()?;
    
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