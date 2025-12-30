use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use crate::benchmark::start_benchmark;
use crate::io::{first_time_setup, get_fabric_loader_versions, get_minecraft_versions, install_fabric_server, InstallerMsg};
use crate::slint_utils::string_vec_to_rc;
use crate::system_info::SystemInfo;
use slint::{Model, SharedString};

mod system_info;
mod io;
mod slint_utils;
mod benchmark;

slint::include_modules!();
fn main() {
    let app = App::new().unwrap();

    let appdata = app.global::<Info>();
    // Initial startup
    first_time_setup();


    // Collect system info
    let info = SystemInfo::get();
    if let Ok(info) = info {
        appdata.set_processor(SharedString::from(info.cpu.trim_end()));
        appdata.set_logical_cpu_processors(info.cores);
        appdata.set_memory_capacity_gb(info.memory.round() as i32);
        appdata.set_graphics_processor(SharedString::from(info.gpus));
        appdata.set_os(SharedString::from(info.os));
    }

    // Populate MC versions
    let mc_vers = get_minecraft_versions();
    appdata.set_stable_minecraft_versions(string_vec_to_rc(&mc_vers));
    appdata.set_selected_minecraft_version(SharedString::from(mc_vers[0].clone()));

    // Populate Fabric versions
    let fabric_vers = get_fabric_loader_versions();
    appdata.set_stable_fabric_loader_versions(string_vec_to_rc(&fabric_vers));
    appdata.set_selected_fabric_loader_version(SharedString::from(fabric_vers[0].clone()));

    // Callbacks
    let callbacks = app.global::<Callbacks>();
    let weak_app_root = app.as_weak();

    callbacks.on_run_benchmark({
        let weak_app_root = weak_app_root.clone();

        move || {
            let (tx, rx) = mpsc::channel::<InstallerMsg>();

            let weak_app_ui = weak_app_root.clone();
            let app = weak_app_ui.upgrade().unwrap();

            let (mc_ver, fabric_ver, jvms, ram) = {
                let jvms = app.global::<Info>().get_jvms();
                (
                    app.global::<Info>().get_selected_minecraft_version(),
                    app.global::<Info>().get_selected_fabric_loader_version(),
                    jvms.iter().map(|j| j.to_string()).collect::<Vec<_>>(),
                    app.global::<Info>().get_ram_alloc() as u32,
                )
            };

            app.global::<Info>().set_status(BenchmarkingStatus::Install);

            // Worker thread
            thread::spawn({
                let tx = tx.clone();
                move || {
                    if install_fabric_server(&mc_ver, &fabric_ver, jvms, ram, &tx).is_err() {
                        return;
                    }

                    tx.send(InstallerMsg::Status(BenchmarkingStatus::Running)).ok();
                    start_benchmark(&tx);
                }
            });

            // UI timer
            let weak_app_timer = weak_app_root.clone();
            let timer = Rc::new(RefCell::new(slint::Timer::default()));

            let timer_for_cb = timer.clone();

            timer.borrow().start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(50),
                move || {
                    if let Some(app) = weak_app_timer.upgrade() {
                        while let Ok(msg) = rx.try_recv() {
                            match msg {
                                InstallerMsg::Progress(p) => {
                                    app.global::<Info>().set_installing(p);
                                }
                                InstallerMsg::Status(s) => {
                                    app.global::<Info>().set_status(s);

                                    if s == BenchmarkingStatus::Finished {
                                        timer_for_cb.borrow().stop();
                                        return;
                                    }
                                }
                                InstallerMsg::Error(e) => {
                                    eprintln!("Installer error: {}", e);
                                }
                                InstallerMsg::InstallingMsg(s) => {
                                    app.global::<Info>().set_installer_msg(SharedString::from(s));
                                }
                            }
                        }
                    } else {
                        timer_for_cb.borrow().stop();
                    }
                },
            );
        }
    });

    app.run().unwrap();
}
