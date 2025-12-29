use slint::SharedString;
use crate::system_info::{CPUMemoryInfo, GPUInfo, OSInfo};

mod system_info;

slint::include_modules!();
fn main() {
    let app = App::new().unwrap();

    let appdata = app.global::<Info>();

    // Collect system info
    let gpu_info = GPUInfo::collect();
    let cpu_info = CPUMemoryInfo::collect();
    appdata.set_processor(SharedString::from(cpu_info.name.trim_end()));
    appdata.set_logical_cpu_processors(cpu_info.logical_cores as i32);
    let mib_memory = cpu_info.memory as f64 / 1049000_f64;
    appdata.set_memory_capacity_mib(mib_memory.round() as i32);
    let mut proper_gpu: Option<GPUInfo> = None;
    for gpu in gpu_info {
        if let Some (current_best) = proper_gpu.clone() {
            if current_best.cores <= gpu.cores {
                proper_gpu = Some(gpu);
            }
        } else {
            proper_gpu = Some(gpu);
        }
    }
    if let Some(gpu) = proper_gpu {
        appdata.set_graphics_processor(SharedString::from(gpu.name));
        appdata.set_logical_gpu_processors(gpu.cores as i32);
    }
    let os_info = OSInfo::collect();
    appdata.set_os(SharedString::from(os_info.to_string()));

    app.run().unwrap();
}
