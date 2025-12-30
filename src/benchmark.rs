use crate::io::{java_dir, server_dir, InstallerMsg};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

pub fn start_benchmark(sender: &Sender<InstallerMsg>) {

}

pub fn launch_jar(version: String, jvm: String, memory: u32, args: Vec<String>) {
    let jvm_path: PathBuf = if cfg!(target_os = "windows") {
        java_dir()
            .join(jvm.to_lowercase())
            .join("bin")
            .join("javaw.exe")
    } else {
        java_dir()
            .join(jvm.to_lowercase())
            .join("bin")
            .join("java")
    };

    let jar_name = "fabric-server.jar";

    let mut command = Command::new(jvm_path);

    command.arg(format!("-Xms{}G", memory));
    command.arg(format!("-Xmx{}G", memory));

    command.args(args);
    command.arg("-jar");
    command.arg(jar_name);

    command.current_dir(server_dir().join(version));

    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    match command.spawn() {
        Ok(mut child) => {
            let _ = child.wait();
        }
        Err(e) => {
            eprintln!("Failed to launch server: {}", e);
        }
    }
}