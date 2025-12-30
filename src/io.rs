use std::{env, fs, io};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use reqwest::blocking::Client;
use slint::{ComponentHandle, Model};
use zip::ZipArchive;
use serde::Deserialize;
use crate::{App, BenchmarkingStatus, Info};
use crate::benchmark::launch_jar;

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
pub fn install_fabric_server(mc_ver: &str, fabric_ver: &str, jvms: Vec<String>, ram: u32, sender: &Sender<InstallerMsg>) -> io::Result<()> {
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
    let progress_per_jvm = 0.4 / jvms.iter().len() as f32;
    let mut progress = 0.25;
    for jvm in jvms.iter() {
        if !java_installed(jvm.as_str()) {
            sender.send(InstallerMsg::InstallingMsg(format!("Installing {} JVM", jvm))).ok();
            install_java(jvm.as_str())?;
        }
        progress += progress_per_jvm;
        sender.send(InstallerMsg::Progress(progress)).ok();
    }
    sender.send(InstallerMsg::Progress(0.65)).ok();

    // Run until EULA
    if !eula_exists(mc_ver.to_string()) {
        sender.send(InstallerMsg::InstallingMsg("Installing Minecraft Libraries".to_string())).ok();
        launch_jar(mc_ver.to_string(), jvms.iter().nth(0).unwrap().to_string(), ram, vec![]);
        write_eula(mc_ver.to_string());
    }

    sender.send(InstallerMsg::Progress(1.0)).ok();
    Ok(())
}

// Installing JVMs
fn java_installed(distro: &str) -> bool {
    fs::exists(java_dir().join(&*distro.to_lowercase())).unwrap_or(false)
}

pub fn install_java(distro: &str) -> io::Result<()> {
    let url = match distro {
        "Azul" => {
            let api_url = azul_url();
            println!("Requesting Azul API JSON from {}", api_url);

            let client = Client::new();
            let resp = client
                .get(&api_url)
                .send()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            if !resp.status().is_success() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("API request failed: {}", resp.status()),
                ));
            }

            let azul_json: AzulJson = resp
                .json()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            azul_json.url
        }
        "Adoptium" => adoptium_url(),
        "Graalvm" => graalvm_url(),
        _ => panic!("Unknown JVM distro"),
    };

    println!("Downloading {}", url);

    let response = reqwest::blocking::get(&url)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let bytes = response
        .bytes()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let filename = format!("{}/{}.zip", java_dir().to_str().unwrap(), distro.to_lowercase());
    println!("Saving to {}", filename);
    let mut file = File::create(&filename)?;
    file.write_all(&bytes)?;

    println!(
        "Extracting {}",
        java_dir().join(distro.to_lowercase()).to_str().unwrap()
    );
    extract_zip(
        Path::new(&filename),
        &java_dir().join(distro.to_lowercase()),
    )?;

    Ok(())
}

fn platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        panic!("Unsupported OS");
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        panic!("Unsupported architecture");
    };

    (os, arch)
}

#[derive(Debug, Deserialize)]
struct AzulJson {
    url: String,
}

fn azul_url() -> String {
    let (os, arch) = platform();
    format!(
        "https://api.azul.com/zulu/download/community/v1.0/bundles/latest?java_version=25&os={}&arch={}&ext=zip&bundle_type=jdk",
        os, arch
    )
}

fn adoptium_url() -> String {
    let (os, arch) = platform();

    format!(
        "https://api.adoptium.net/v3/binary/latest/25/ga/{}/{}/jdk/hotspot/normal/eclipse?project=jdk",
        os, arch
    )
}

fn graalvm_url() -> String {
    let (os, arch) = platform();

    let os_str = match os {
        "windows" => "windows",
        "linux" => "linux",
        "mac" => "darwin",
        _ => unreachable!(),
    };

    let arch_str = match arch {
        "x64" => "x64",
        "aarch64" => "aarch64",
        _ => unreachable!(),
    };

    // GraalVM 25 release version
    let version = "25.0.1";

    // Correct URL pattern
    format!(
        "https://github.com/graalvm/graalvm-ce-builds/releases/download/jdk-{version}/graalvm-community-jdk-{version}_{os_str}-{arch_str}_bin.zip",
        version = version,
        os_str = os_str,
        arch_str = arch_str,
    )
}

fn extract_zip(zip_path: &Path, output_dir: &Path) -> io::Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Determine top-level folder name to strip
    let mut top_level = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let path = entry.mangled_name();
        if let Some(first) = path.iter().next() {
            top_level = Some(first.to_owned());
            break;
        }
    }
    top_level.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Empty zip"))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut out_path = PathBuf::new();


        let name = entry.mangled_name();
        let mut components = name.components();
        components.next();
        for comp in components {
            out_path.push(comp.as_os_str());
        }

        let out_path = output_dir.join(out_path);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut outfile = File::create(&out_path)?;
            io::copy(&mut entry, &mut outfile)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = entry.unix_mode() {
                    fs::set_permissions(&out_path, fs::Permissions::from_mode(mode))?;
                }
            }
        }
    }

    // Delete the zip after extraction
    fs::remove_file(zip_path)?;

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