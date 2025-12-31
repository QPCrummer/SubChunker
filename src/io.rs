use crate::benchmark::launch_jar;
use crate::java::{install_java, java_installed};
use crate::BenchmarkingStatus;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::{env, fs, io};

pub const MAIN_DIR: &str = "subchunker";
pub const SERVER_DIR: &str = "subchunker/server";
pub const JAVA_DIR: &str = "subchunker/java";
pub const DATA_DIR: &str = "subchunker/data";
pub const RUNS_FILE: &str = "subchunker/data/benchmarks.json";
pub fn first_time_setup() {
    if fs::exists(MAIN_DIR).unwrap_or(false) {
        // Not the first startup
        return;
    }

    // Create folders
    fs::create_dir(MAIN_DIR).unwrap();
    fs::create_dir(SERVER_DIR).unwrap();
    fs::create_dir(DATA_DIR).unwrap();
    fs::create_dir(JAVA_DIR).unwrap();
}

fn working_dir() -> PathBuf {
    env::current_dir().unwrap()
}

pub fn main_dir() -> PathBuf {
    working_dir().join(MAIN_DIR)
}

pub fn server_dir() -> PathBuf {
    working_dir().join(SERVER_DIR)
}

pub fn data_dir() -> PathBuf {
    working_dir().join(DATA_DIR)
}
pub fn java_dir() -> PathBuf {
    working_dir().join(JAVA_DIR)
}

pub enum InstallerMsg {
    Progress(f32),
    Status(BenchmarkingStatus),
    InstallingMsg(String),
    Error(String),
}

// Installing
pub fn install_fabric_server(mc_ver: &str, fabric_ver: &str, jvm: &str, ram: u32, sender: &Sender<InstallerMsg>) -> io::Result<()> {
    // Install MC
    if !mc_ver_installed(mc_ver.to_string()) {
        sender.send(InstallerMsg::InstallingMsg(format!("Installing Minecraft {}", mc_ver))).ok();
        let url = format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/1.1.0/server/jar",
            mc_ver, fabric_ver
        );

        let response = reqwest::blocking::get(&url)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        if !response.status().is_success() {
            println!("Download failed");
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Download failed: {}", response.status()),
            ));
        }

        fs::create_dir(server_dir().join(mc_ver))?;

        let output_path = server_dir().join(mc_ver).join("fabric-server.jar");
        let mut file = File::create(output_path)?;

        let bytes = response.bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        file.write_all(&bytes)?;
    }
    sender.send(InstallerMsg::Progress(0.25)).ok();

    // Install Java

    if !java_installed(jvm) {
        sender.send(InstallerMsg::InstallingMsg(format!("Installing {} JVM", jvm))).ok();
        install_java(jvm)?;
    }
    sender.send(InstallerMsg::Progress(0.65)).ok();

    // Run until EULA
    if !eula_exists(mc_ver.to_string()) {
        sender.send(InstallerMsg::InstallingMsg("Installing Minecraft Libraries".to_string())).ok();
        launch_jar(mc_ver.to_string(), jvm.to_string(), ram, vec![], None);
        write_eula(mc_ver.to_string());
    }

    sender.send(InstallerMsg::Progress(1.0)).ok();
    Ok(())
}


// Info functions
fn installed_minecraft_versions() -> Vec<String> {
    let mut output: Vec<String> = Vec::new();
    for entry in server_dir().read_dir().expect("Failed to read server dir") {
        if let Ok(entry) = entry {
            output.push(entry.file_name().to_str().unwrap().to_string());
        }
    }
    output
}

pub fn get_minecraft_versions() -> Vec<String> {
    let url: &str = "https://meta.fabricmc.net/v2/versions/game";
    let versions = reqwest::blocking::get(url).unwrap();
    let json: serde_json::Value = versions.json().unwrap();
    let mut output: Vec<String> = Vec::new();
    for version in json.as_array().unwrap() {
        let stable: bool = version["stable"].as_bool().unwrap();
        if stable {
            let version = version["version"].as_str().unwrap();
            output.push(version.to_string());
        }
    }
    output
}

pub fn get_fabric_loader_versions() -> Vec<String> {
    let url: &str = "https://meta.fabricmc.net/v2/versions/loader";
    let versions = reqwest::blocking::get(url).unwrap();
    let json: serde_json::Value = versions.json().unwrap();
    let mut output: Vec<String> = Vec::new();
    for version in json.as_array().unwrap() {
        let stable: bool = version["stable"].as_bool().unwrap();
        if stable {
            let version = version["version"].as_str().unwrap();
            output.push(version.to_string());
        }
    }
    output
}

pub fn mc_ver_installed(version: String) -> bool {
    let installed_minecraft_versions = installed_minecraft_versions();
    installed_minecraft_versions.contains(&version)
}

fn eula_exists(version: String) -> bool {
    fs::exists(server_dir().join(&*version).join("eula.txt")).unwrap()
}

fn write_eula(version: String) {
    if eula_exists(version.clone()) {
        accept_minecraft_eula(&*server_dir().join(&*version).join("eula.txt")).unwrap();
    }
}

fn accept_minecraft_eula(path: &Path) -> io::Result<()> {
    let contents = fs::read_to_string(path)?;

    let updated = contents
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("eula=") {
                "eula=true".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(path, updated)?;
    Ok(())
}