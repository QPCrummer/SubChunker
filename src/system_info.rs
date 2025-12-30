use hwinfo_rs::hwinfo;

pub struct SystemInfo {
    pub os: String,
    pub cpu: String,
    pub cores: i32,
    pub gpus: String,
    pub memory: f64,
}

impl SystemInfo {
    pub fn get() -> hwinfo::Result<SystemInfo> {
        let os = hwinfo::os_info()?;
        let mem = hwinfo::memory_info()?;
        let cpus_vec = hwinfo::cpus()?;
        let cpu: String = if cpus_vec.len() > 1 {
            cpus_vec[0].model_name.clone() + " x" + cpus_vec.len().to_string().as_str()
        } else {
            cpus_vec[0].model_name.clone()
        };
        let cores: i32 = cpus_vec.iter().map(|cpu| cpu.num_logical_cores).sum();
        let gpus_vec = hwinfo::gpus()?;
        let mut gpus: String = String::new();
        for gpu in gpus_vec {
            if !gpus.is_empty() {
                gpus.push_str("\n");
            }
            if gpu.num_cores != 0 {
                gpus.push_str(&*(gpu.name + " (" + gpu.num_cores.to_string().as_str() + ")"));
            } else {
                gpus.push_str(&*(gpu.name));
            }
        }

        Ok(SystemInfo {
            os: os.name,
            cpu,
            cores,
            gpus,
            memory: bytes_to_gb(mem.total_bytes)
        })
    }
}

fn bytes_to_gb(bytes: i64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}