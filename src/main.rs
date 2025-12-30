use crate::io::{first_time_setup, get_fabric_loader_versions, get_minecraft_versions, install_fabric_server, server_dir, InstallerMsg};
use crate::mods::{get_url, MODS};
use crate::slint_utils::{bool_arr_to_rc, string_arr_to_rc, string_vec_to_rc};
use crate::system_info::SystemInfo;
use slint::{Model, SharedString};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

mod system_info;
mod io;
mod slint_utils;
mod benchmark;
mod mods;

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
    
    // Populate Mods
    appdata.set_recommended_mod_list(string_arr_to_rc(&MODS));
    appdata.set_recommended_mods_toggle(bool_arr_to_rc(&[true; MODS.len()]));

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

                    // Begin installing mods
                    tx.send(InstallerMsg::Status(BenchmarkingStatus::InstallMod)).ok();
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

                                    if s == BenchmarkingStatus::InstallMod {
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

    callbacks.on_install_mod({
        let weak_app_root = weak_app_root.clone();
        let mc_ver = app.global::<Info>().get_selected_minecraft_version();
        move |string| {
            let url = get_url(string.to_string(), mc_ver.to_string());
            // TODO Open Browser
        }
    });
    
    callbacks.on_open_mods_folder({
        let weak_app_root = weak_app_root.clone();
        let mc_ver = app.global::<Info>().get_selected_minecraft_version();
        move || {
            let directory = server_dir().join(mc_ver.to_string()).join("mods");
            // TODO Open File Explorer
        }
    });
    
    callbacks.on_next_mod({
        let weak_app_root = weak_app_root.clone().unwrap();
        let index = app.global::<Info>().get_current_mod_download_index();
        move || {
            let mut next_index = index + 1;
            while !weak_app_root.global::<Info>().get_recommended_mods_toggle().iter().nth(next_index as usize).unwrap() {
                next_index += 1;
                if next_index >= MODS.len() as i32 {
                    // TODO Install Chunky
                    
                    // TODO Begin benchmark
                    weak_app_root.global::<Info>().set_status(BenchmarkingStatus::Running);
                    // start_benchmark(&tx);
                    return;
                }
            }
          
            weak_app_root.global::<Info>().set_current_mod_download_index(next_index);
            let next_mod = MODS.get(next_index as usize).unwrap();
            weak_app_root.global::<Info>().set_current_mod_download(SharedString::from(*next_mod));
        }
    });

    app.run().unwrap();
}
