use std::sync::{Mutex, OnceLock};

use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::ResourceUsage;

static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

fn system_snapshot() -> &'static Mutex<System> {
    SYSTEM.get_or_init(|| Mutex::new(System::new()))
}

pub fn get_resource_usage() -> Result<ResourceUsage, String> {
    let pid =
        get_current_pid().map_err(|error| format!("No se pudo identificar Columnia: {error}"))?;
    let mut system = system_snapshot()
        .lock()
        .map_err(|_| "El monitor de recursos quedó bloqueado.".to_owned())?;

    system.refresh_memory();
    system.refresh_cpu_usage();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().with_memory().with_cpu(),
    );

    let process = system
        .process(pid)
        .ok_or_else(|| "Columnia terminó antes de leer su consumo.".to_owned())?;

    Ok(ResourceUsage {
        process_cpu_percentage: process.cpu_usage(),
        system_cpu_percentage: system.global_cpu_usage(),
        process_memory_bytes: process.memory(),
        system_memory_used_bytes: system.used_memory(),
        system_memory_total_bytes: system.total_memory(),
    })
}
