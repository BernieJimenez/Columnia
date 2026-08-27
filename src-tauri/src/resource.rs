use std::sync::{Mutex, OnceLock};

use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::{GpuStatus, GpuUsage, ResourceUsage};

static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

fn gpu_snapshot() -> GpuUsage {
    // No GPU execution backend is initialized today. Keep this as a typed
    // capability boundary so a future Windows probe can be added without
    // changing the IPC shape or presenting fabricated measurements.
    GpuUsage {
        status: GpuStatus::Unavailable,
        usage_percentage: None,
        memory_used_bytes: None,
        memory_total_bytes: None,
        reason: Some("Columnia usa CPU; no hay una sonda GPU disponible."),
    }
}

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
        logical_cpu_count: system.cpus().len().max(1),
        process_memory_bytes: process.memory(),
        system_memory_used_bytes: system.used_memory(),
        system_memory_total_bytes: system.total_memory(),
        system_memory_available_bytes: Some(system.available_memory()),
        gpu: Some(gpu_snapshot()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_snapshot_is_explicitly_unavailable_without_fabricated_values() {
        let gpu = gpu_snapshot();

        assert_eq!(gpu.status, GpuStatus::Unavailable);
        assert!(gpu.usage_percentage.is_none());
        assert!(gpu.memory_used_bytes.is_none());
        assert!(gpu.memory_total_bytes.is_none());
        assert!(gpu.reason.is_some());
    }
}
