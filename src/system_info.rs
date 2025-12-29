use rust_gpu_tools::Device;
use sysinfo::System;

#[derive(Clone)]
pub struct GPUInfo {
    pub name: String,
    pub cores: u32,
    pub ocl_capable: bool,
}

impl GPUInfo {
    pub fn collect() -> Vec<Self> {
        let mut gpu_info = Vec::new();
        for gpu in Device::all() {
            let info = GPUInfo {
                name: gpu.name(),
                cores: gpu.compute_units(),
                ocl_capable: gpu.opencl_device().is_some(),
            };
            gpu_info.push(info);
        }
        gpu_info
    }
}

pub struct CPUMemoryInfo {
    pub name: String,
    pub logical_cores: u32,
    pub memory: u64,
}

impl CPUMemoryInfo {
    pub fn collect() -> Self {
        let sys = System::new_all();
        CPUMemoryInfo {
            name: sys.cpus()[0].brand().to_string(),
            logical_cores: sys.cpus().len() as u32,
            memory: sys.total_memory()
        }
    }
}

pub struct OSInfo {
    pub os_type: String,
    pub os_version: String,
    pub os_architecture: String,
    pub os_bitness: String,
}

impl OSInfo {
    pub fn collect() -> Self {
        let os = os_info::get();
        OSInfo {
            os_type: os.os_type().to_string(),
            os_version: os.version().to_string(),
            os_bitness: os.bitness().to_string(),
            os_architecture: os.architecture().unwrap_or_default().to_string(),
        }
    }

    pub fn to_string(&self) -> String {
        self.os_type.clone() + " " + &*self.os_version + " " + &*self.os_architecture + " " + &*self.os_bitness
    }
}

