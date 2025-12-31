use crate::io::{java_dir, server_dir};
use crate::{App, BenchmarkingStatus, Info};
use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::{fs, thread};
use slint::{ComponentHandle, Weak};

pub enum RunningMsg {
    Progress(f32),
    Result(f32),
}

pub fn start_benchmark(app: &Weak<App>) {
    let (tx, rx) = mpsc::channel::<RunningMsg>();
    let weak_app_timer = app.clone();
    let non_weak_app = app.unwrap();

    let mc_ver = non_weak_app.global::<Info>().get_selected_minecraft_version();
    let jvm = non_weak_app.global::<Info>().get_selected_jvm();
    let memory = non_weak_app.global::<Info>().get_memory_capacity_gb();
    // TODO Parse JVM Flags

    thread::spawn({
        let tx = tx.clone();
        move || {
            launch_jar(mc_ver.to_string(), jvm.to_string(), memory as u32, vec![], Some(tx));
        }
    });

    // UI timer
    let timer = Rc::new(RefCell::new(slint::Timer::default()));

    let timer_for_cb = timer.clone();

    let mut running_avg = RunningAverage::new();

    timer.borrow().start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(50),
        move || {
            if let Some(app) = weak_app_timer.upgrade() {
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        RunningMsg::Progress(p) => {
                            app.global::<Info>().set_progress(p);

                            if p == 1.0 {
                                let average_cps = running_avg.average();
                                // TODO Process Average
                                // TODO Finish
                                app.global::<Info>().set_status(BenchmarkingStatus::Finished);
                            }
                        }
                        RunningMsg::Result(r) => {
                            running_avg.add(r);
                        }
                    }
                }
            } else {
                timer_for_cb.borrow().stop();
            }
        },
    );
}

pub fn launch_jar(version: String, jvm: String, memory: u32, args: Vec<String>, tx: Option<Sender<RunningMsg>>) {
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
    command.arg("nogui");

    command.current_dir(server_dir().join(&version));

    if let Some (tx) = tx {
        command
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        match command.spawn() {
            Ok(mut child) => {
                if let Some(stdout) = child.stdout.take() {
                    let tx_clone = tx.clone();
                    thread::spawn(move || {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().flatten() {
                            println!("{}", line); // TODO Remove Debug
                            parse_console(line, &tx_clone)
                        }
                    });

                    let _ = child.wait();
                }
            }
            Err(e) => {
                eprintln!("Failed to launch server: {}", e);
            }
        }
    } else {
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

    // Remove world
    let world_path = server_dir().join(version).join("world");
    if fs::exists(&world_path).unwrap() {
        fs::remove_dir_all(world_path).unwrap();
    }
}

fn parse_console(line: String,  tx: &Sender<RunningMsg>) {
    if line.contains("[Chunky]") {
        if line.contains("%") {
            let split1 = line.split('(').last().unwrap();
            let split2 = split1.split('%').nth(0).unwrap();
            let percent_complete = f32::from_str(split2).unwrap() / 100.0;
            tx.send(RunningMsg::Progress(percent_complete)).unwrap();
            if percent_complete != 1.0 {
                let split3 = split1.split(':').nth(1).unwrap().trim_start();
                let split4 = split3.split(' ').nth(0).unwrap();
                let cps = f32::from_str(split4).unwrap();
                tx.send(RunningMsg::Result(cps)).unwrap();
            }
        }
    }
}

#[derive(Default)]
pub struct RunningAverage {
    count: u32,
    avg: f32,
}

impl RunningAverage {
    pub fn new() -> Self {
        Self {
            count: 0,
            avg: 0.0,
        }
    }

    pub fn add(&mut self, value: f32) -> f32 {
        self.count += 1;
        self.avg += (value - self.avg) / self.count as f32;
        self.avg
    }

    pub fn average(&self) -> f32 {
        self.avg
    }
}