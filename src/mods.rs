use std::{fs, io};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use serde::Deserialize;
use zip::ZipArchive;
use crate::io::server_dir;

// Mods
pub const MODS: [&str; 7] = ["Lithium", "Fabric-Api", "Ferritecore", "C2me", "Servercore",
                                 "Structure_Layout_Optimizer", "ScalableLux"];
pub const MOD_URLS: [&str; 7] = ["https://modrinth.com/mod/lithium/versions", "https://modrinth.com/mod/fabric-api/versions",
                                     "https://modrinth.com/mod/ferrite-core/versions", "https://modrinth.com/mod/c2me-fabric/versions",
                                     "https://modrinth.com/mod/servercore/versions", "https://modrinth.com/mod/structure-layout-optimizer/versions",
                                     "https://modrinth.com/mod/scalablelux/versions"];
pub const CHUNKY_URL: &str = "https://modrinth.com/plugin/chunky/versions";


#[derive(Deserialize)]
struct FabricModJson {
    id: String,
}

pub fn get_mods(version: String) -> Vec<String> {
    let mut output: Vec<String> = Vec::new();
    let mods_folder = server_dir().join(&*version).join("mods");
    if mods_folder.exists() {
        for entry in fs::read_dir(&mods_folder).unwrap() {
            if let Ok(entry) = entry {
                if entry.file_name().to_str().unwrap().ends_with(".jar") {
                    output.push(get_fabric_mod_id(entry.path()).unwrap());
                }
            }
        }
    }
    output
}

fn get_fabric_mod_id<P: AsRef<Path>>(jar_path: P) -> io::Result<String> {
    let file = File::open(jar_path)?;
    let mut zip = ZipArchive::new(file)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut mod_json = zip
        .by_name("fabric.mod.json")
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "fabric.mod.json not found in JAR",
            )
        })?;

    let mut json_str = String::new();
    mod_json.read_to_string(&mut json_str)?;

    let parsed: FabricModJson = serde_json::from_str(&json_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(parsed.id)
}

pub fn get_url(mod_name: String, version: String) -> String {
    let index = MODS.iter().position(|x| x == &mod_name).unwrap();
    format!("{}?g={}", MOD_URLS[index], version)
}