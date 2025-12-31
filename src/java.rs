use std::{fs, io};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use reqwest::blocking::Client;
use serde::Deserialize;
use zip::ZipArchive;
use crate::io::java_dir;

// Installing JVMs
pub fn java_installed(distro: &str) -> bool {
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