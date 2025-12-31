use crate::io::{first_time_setup, get_fabric_loader_versions, get_minecraft_versions, install_fabric_server, server_dir, InstallerMsg};
use crate::mods::{get_mods, get_url, is_mod_installed, MODS, REQ_MODS};
use crate::slint_utils::{bool_arr_to_rc, string_arr_to_rc, string_vec_to_rc};
use crate::system_info::SystemInfo;
use slint::{Model, SharedString};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use crate::benchmark::start_benchmark;

mod system_info;
mod io;
mod slint_utils;
mod benchmark;
mod mods;
mod java;

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

    // Populate JVM
    appdata.set_selected_jvm(SharedString::from("Azul"));

    // Callbacks
    let callbacks = app.global::<Callbacks>();
    let master_weak_app = app.as_weak();

    callbacks.on_run_benchmark({
        let weak_app_root = master_weak_app.clone();

        move || {
            let (tx, rx) = mpsc::channel::<InstallerMsg>();

            let weak_app_ui = weak_app_root.clone();
            let app = weak_app_ui.upgrade().unwrap();

            let (mc_ver, fabric_ver, jvm, ram) = {
                (
                    app.global::<Info>().get_selected_minecraft_version(),
                    app.global::<Info>().get_selected_fabric_loader_version(),
                    app.global::<Info>().get_selected_jvm(),
                    app.global::<Info>().get_ram_alloc() as u32,
                )
            };

            app.global::<Info>().set_status(BenchmarkingStatus::Install);

            // Worker thread
            let mc_ver_clone = mc_ver.clone();
            thread::spawn({
                let tx = tx.clone();
                move || {
                    if install_fabric_server(&mc_ver, &fabric_ver, &jvm, ram, &tx).is_err() {
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
                                    app.global::<Info>().set_progress(p);
                                }
                                InstallerMsg::Status(s) => {
                                    if s == BenchmarkingStatus::InstallMod {
                                        app.global::<Info>().set_progress(0.0);
                                        // Check installed mods
                                        let installed_mods = get_mods(mc_ver_clone.to_string());
                                        let mut skip_mod_installs: bool = true;
                                        let mut new_recommended_mods_toggle: [bool; MODS.len()] = [false; MODS.len()];
                                        for (i, should_install) in app.global::<Info>().get_recommended_mods_toggle().iter().enumerate() {
                                            if !should_install {
                                                continue;
                                            }

                                            let mod_to_check = MODS.get(i).unwrap();
                                            if !installed_mods.contains(&mod_to_check.to_lowercase()) {
                                                skip_mod_installs = false;
                                                new_recommended_mods_toggle[i] = true;
                                            }
                                        }

                                        // Check required mods
                                        for req_mod in REQ_MODS {
                                            if !installed_mods.contains(&req_mod.to_lowercase()) {
                                                skip_mod_installs = false;
                                                break;
                                            }
                                        }

                                        // Set first mod to install
                                        if skip_mod_installs {
                                            app.global::<Info>().set_status(BenchmarkingStatus::Running);
                                            println!("Begin Benchmark");
                                            start_benchmark(&weak_app_ui);
                                        } else {
                                            app.global::<Info>().set_recommended_mods_toggle(bool_arr_to_rc(&new_recommended_mods_toggle));
                                            app.global::<Callbacks>().invoke_next_mod();

                                            app.global::<Info>().set_status(s);
                                        }

                                        timer_for_cb.borrow().stop();
                                        return;
                                    }

                                    app.global::<Info>().set_status(s);
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
        let weak_app_root = master_weak_app.clone().unwrap();
        move |string| {
            let mc_ver = weak_app_root.global::<Info>().get_selected_minecraft_version();
            let url = get_url(string.to_string(), mc_ver.to_string());
            webbrowser::open(&url).unwrap();
        }
    });
    
    callbacks.on_open_mods_folder({
        let weak_app_root = master_weak_app.clone().unwrap();
        move || {
            let mc_ver = weak_app_root.global::<Info>().get_selected_minecraft_version();
            let directory = server_dir().join(mc_ver.to_string()).join("mods");
            open::that(directory).unwrap();
        }
    });
    
    callbacks.on_next_mod({
        let weak_app_root = master_weak_app.clone();
        move || {
            let app = weak_app_root.upgrade().unwrap();
            let index = app.global::<Info>().get_current_mod_download_index();
            let mc_ver = app.global::<Info>().get_selected_minecraft_version();
            let mut next_index = index + 1;

            loop {
                if next_index >= MODS.len() as i32 {
                    let req_mods_index = next_index - MODS.len() as i32;
                    if req_mods_index < REQ_MODS.len() as i32 {
                        // Look at required mods
                        let next_mod = REQ_MODS.get(req_mods_index as usize).unwrap();
                        if is_mod_installed(next_mod.to_string(), mc_ver.to_string()) {
                            next_index += 1;
                            app.global::<Info>().set_current_mod_download_index(next_index);
                            continue;
                        } else {
                            app.global::<Info>().set_current_mod_download(SharedString::from(*next_mod));
                            next_index += 1;
                            app.global::<Info>().set_current_mod_download_index(next_index);
                            return;
                        }
                    }

                    app.global::<Info>().set_current_mod_download_index(-1);
                    app.global::<Info>().set_status(BenchmarkingStatus::Running);
                    println!("Begin Benchmark");
                    start_benchmark(&weak_app_root);
                    return;
                }

                if app.global::<Info>().get_recommended_mods_toggle().iter().nth(next_index as usize).unwrap() {
                    break;
                }

                next_index += 1;
            }

            app.global::<Info>().set_current_mod_download_index(next_index);
            let next_mod = MODS.get(next_index as usize).unwrap();
            app.global::<Info>().set_current_mod_download(SharedString::from(*next_mod));
        }
    });

    app.run().unwrap();
}
