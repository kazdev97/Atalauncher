
pub fn count_mods(instance_dir: &std::path::Path) -> usize {
    let mods_dir = instance_dir.join("mods");
    if !mods_dir.exists() {
        return 0;
    }
    std::fs::read_dir(&mods_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.file_type().map(|ft| ft.is_file()).unwrap_or(false)
                        && {
                            let name = e.file_name().to_string_lossy().to_lowercase();
                            name.ends_with(".jar") && !name.ends_with(".disabled")
                        }
                })
                .count()
        })
        .unwrap_or(0)
}

pub fn system_cpu_cores() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4)
}
pub fn system_total_ram_mb() -> u64 {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct MemoryStatusEx {
            dw_length: u32,
            dw_memory_load: u32,
            ull_total_phys: u64,
            ull_avail_phys: u64,
            ull_total_page_file: u64,
            ull_avail_page_file: u64,
            ull_total_virtual: u64,
            ull_avail_virtual: u64,
            ull_avail_extended_virtual: u64,
        }
        let mut stat = MemoryStatusEx {
            dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
            dw_memory_load: 0,
            ull_total_phys: 0,
            ull_avail_phys: 0,
            ull_total_page_file: 0,
            ull_avail_page_file: 0,
            ull_total_virtual: 0,
            ull_avail_virtual: 0,
            ull_avail_extended_virtual: 0,
        };
        extern "system" {
            fn GlobalMemoryStatusEx(stat: *mut MemoryStatusEx) -> i32;
        }
        let ok = unsafe { GlobalMemoryStatusEx(&mut stat) };
        if ok != 0 && stat.ull_total_phys > 0 {
            stat.ull_total_phys / (1024 * 1024)
        } else {
            8192
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        8192
    }
}
#[derive(serde::Serialize)]
pub struct SystemInfo {
    pub cpu_cores: u32,
    pub total_ram_mb: u64,
}

pub fn system_info() -> SystemInfo {
    SystemInfo {
        cpu_cores: system_cpu_cores(),
        total_ram_mb: system_total_ram_mb(),
    }
}
fn default_ram(total_ram_mb: u64, mod_count: usize) -> (u32, u32) {
    let total_gb = (total_ram_mb / 1024).max(8) as f64;
    let alloc_gb = if mod_count >= 150 {
        (total_gb * 0.75).min(12.0)
    } else if mod_count >= 50 {
        (total_gb * 0.60).clamp(4.0, 10.0)
    } else if mod_count >= 15 {
        (total_gb * 0.50).clamp(3.0, 8.0)
    } else {
        (total_gb * 0.40).clamp(2.0, 6.0)
    };
    let max_mb = ((alloc_gb * 1024.0) as u32 / 512) * 512;
    let min_mb = (max_mb / 4).clamp(1024, 4096);
    (min_mb, max_mb)
}
pub fn resolved_ram(instance_dir: &std::path::Path) -> (u32, u32) {
    let mods = count_mods(instance_dir);
    let (fallback_min, fallback_max) = default_ram(system_total_ram_mb(), mods);
    match crate::sync::load_manifest_cache(instance_dir) {
        Some(m) => {
            let min = if m.ram_min_mb > 0 { m.ram_min_mb } else { fallback_min };
            let max = if m.ram_max_mb > 0 { m.ram_max_mb } else { fallback_max };
            (min, max.min(system_total_ram_mb() as u32))
        }
        None => (fallback_min, fallback_max),
    }
}
#[derive(serde::Serialize)]
pub struct RamRecommend {
    pub total_ram_gb: u32,
    pub min_mb: u32,
    pub max_mb: u32,
}

pub fn ram_recommend(instance_name: &str) -> Result<RamRecommend, String> {
    let dir = crate::instances::instance_path(instance_name);
    if !dir.exists() {
        return Err("La instancia no existe".to_string());
    }
    let (min_mb, max_mb) = resolved_ram(&dir);
    Ok(RamRecommend {
        total_ram_gb: (system_total_ram_mb() / 1024).max(1) as u32,
        min_mb,
        max_mb,
    })
}
pub fn effective_max_ram(instance_dir: &std::path::Path) -> (u32, u32) {
    let (mut min_mb, mut max_mb) = resolved_ram(instance_dir);
    let settings = crate::config::Settings::load();
    if settings.max_ram_mb > 0 {
        max_mb = settings.max_ram_mb.min(system_total_ram_mb() as u32);
    }
    if max_mb < 1024 {
        max_mb = 1024;
    } else if max_mb > system_total_ram_mb() as u32 {
        max_mb = system_total_ram_mb() as u32;
    }
    if min_mb > max_mb {
        min_mb = (max_mb / 2).max(256);
    }
    (min_mb, max_mb)
}
pub fn generate_jvm_flags(
    java_major: u32,
    memory_gb: u32,
    mod_count: usize,
    streamer_mode: bool,
) -> Vec<String> {
    let mut flags = Vec::new();
    let cpu_cores = system_cpu_cores();
    let use_zgc = java_major >= 21 && memory_gb >= 12 && mod_count >= 150;
    if use_zgc {
        flags.push("-XX:+UseZGC".to_string());
        flags.push("-XX:+ZGenerational".to_string());
    } else {
        flags.push("-XX:+UseG1GC".to_string());
    }
    let (parallel_threads, conc_threads) = if streamer_mode {
        if cpu_cores <= 4 {
            (2.max(cpu_cores - 1), 1)
        } else if cpu_cores <= 8 {
            (2.max(cpu_cores - 2), 2)
        } else if cpu_cores <= 16 {
            let p = 4.max(cpu_cores - 4);
            (p, (2).max(p / 4))
        } else {
            let p = 6.max(cpu_cores - 6);
            (p, (2).max(p / 4))
        }
    } else {
        let p = ((cpu_cores as f64 * 0.75) as u32).max(2);
        (p, (p / 4).max(1))
    };
    flags.push(format!("-XX:ParallelGCThreads={}", parallel_threads));
    flags.push(format!("-XX:ConcGCThreads={}", conc_threads));
    if !use_zgc {
        flags.push("-XX:+ParallelRefProcEnabled".to_string());
        flags.push("-XX:MaxGCPauseMillis=200".to_string());
        flags.push("-XX:+UnlockExperimentalVMOptions".to_string());
        flags.push("-XX:+DisableExplicitGC".to_string());

        if memory_gb <= 4 {
            flags.push("-XX:G1NewSizePercent=25".to_string());
            flags.push("-XX:G1MaxNewSizePercent=35".to_string());
            flags.push("-XX:G1HeapRegionSize=8M".to_string());
            flags.push("-XX:G1ReservePercent=15".to_string());
            flags.push("-XX:InitiatingHeapOccupancyPercent=20".to_string());
        } else if memory_gb <= 8 {
            flags.push("-XX:G1NewSizePercent=30".to_string());
            flags.push("-XX:G1MaxNewSizePercent=40".to_string());
            flags.push("-XX:G1HeapRegionSize=16M".to_string());
            flags.push("-XX:G1ReservePercent=20".to_string());
            flags.push("-XX:InitiatingHeapOccupancyPercent=15".to_string());
        } else {
            flags.push("-XX:G1NewSizePercent=35".to_string());
            flags.push("-XX:G1MaxNewSizePercent=45".to_string());
            flags.push("-XX:G1HeapRegionSize=32M".to_string());
            flags.push("-XX:G1ReservePercent=20".to_string());
            flags.push("-XX:InitiatingHeapOccupancyPercent=15".to_string());
        }
        flags.push("-XX:G1HeapWastePercent=5".to_string());
        flags.push("-XX:G1MixedGCCountTarget=4".to_string());
        flags.push("-XX:G1MixedGCLiveThresholdPercent=90".to_string());
        flags.push("-XX:G1RSetUpdatingPauseTimePercent=5".to_string());
        flags.push("-XX:SurvivorRatio=32".to_string());
        flags.push("-XX:MaxTenuringThreshold=1".to_string());
    }
    flags.push("-XX:+PerfDisableSharedMem".to_string());
    flags.push("-XX:+UseCompressedOops".to_string());
    flags.push("-XX:+UseCompressedClassPointers".to_string());
    flags.push("-XX:+UseStringDeduplication".to_string());
    flags.push("-XX:+OptimizeStringConcat".to_string());
    if mod_count >= 50 {
        flags.push("-XX:MetaspaceSize=256M".to_string());
        flags.push("-XX:MaxMetaspaceSize=768M".to_string());
    } else if mod_count >= 15 {
        flags.push("-XX:MetaspaceSize=128M".to_string());
        flags.push("-XX:MaxMetaspaceSize=512M".to_string());
    }
    if java_major <= 8 {
        flags.push("-XX:+UseFastUnorderedTimeStamps".to_string());
    }
    if java_major >= 17 {
        flags.push("-XX:+EnableVectorSupport".to_string());
        flags.push("-XX:+UseFMA".to_string());
    }
    flags.push("-Djava.net.preferIPv4Stack=true".to_string());
    flags.push("-Dfile.encoding=UTF-8".to_string());
    flags.push("-Djava.awt.headless=false".to_string());
    flags.push("-Dfml.ignoreInvalidMinecraftCertificates=true".to_string());
    flags.push("-Dfml.ignorePatchDiscrepancies=true".to_string());
    flags
}