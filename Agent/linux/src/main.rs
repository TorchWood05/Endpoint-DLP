#![allow(warnings)]
use nix::sys::utsname::uname;
use tracing::{debug, error, info, instrument};

#[derive(Debug)]
struct Systeminfo {
    kernel: (u32, u32),
    os: String,
    distro_id: String,
    architecture: String,
    fw_version: String,
}

fn system_info() {
    let kernel_version = uname()
        .expect("Failed to get kernel version");
    let kernel_version = kernel_version.release()
        .to_string_lossy();
    println!("{}", kernel_version);
    let version = kernel_version
        .split(|c: char| c == '-' || c == '+')
        .next()
        .unwrap_or(&kernel_version);
    println!("{}", version);
}

fn main() {
    tracing_subscriber::fmt::init();
    system_info();
}
