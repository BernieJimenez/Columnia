use std::sync::{Mutex, OnceLock};
use std::thread;

use rayon::ThreadPoolBuilder;
use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::{GpuStatus, GpuUsage, ResourceUsage};

static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();
static PERFORMANCE: OnceLock<Mutex<PerformanceState>> = OnceLock::new();

const MAX_THREADS: usize = 64;

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PerformanceProfile {
    Conservative,
    Balanced,
    Maximum,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSettings {
    /// Profile requested by the user. It can differ from the active profile
    /// when Rayon was initialized before the setting could be applied.
    pub requested_profile: PerformanceProfile,
    pub active_profile: Option<PerformanceProfile>,
    pub requested_threads: usize,
    pub active_threads: Option<usize>,
    pub applied: bool,
    pub locked: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
struct PerformanceState {
    requested_profile: PerformanceProfile,
    active_profile: Option<PerformanceProfile>,
    requested_threads: usize,
    active_threads: Option<usize>,
    applied: bool,
    locked: bool,
    reason: Option<String>,
}

fn logical_cpu_count() -> usize {
    thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .clamp(1, MAX_THREADS)
}

fn profile_thread_count(profile: PerformanceProfile, logical_cpus: usize) -> usize {
    let logical_cpus = logical_cpus.clamp(1, MAX_THREADS);
    match profile {
        PerformanceProfile::Conservative => 1,
        PerformanceProfile::Balanced => logical_cpus.div_ceil(2),
        PerformanceProfile::Maximum => logical_cpus,
    }
}

fn performance_state() -> &'static Mutex<PerformanceState> {
    PERFORMANCE.get_or_init(|| {
        let profile = PerformanceProfile::Balanced;
        let threads = profile_thread_count(profile, logical_cpu_count());
        Mutex::new(PerformanceState {
            requested_profile: profile,
            active_profile: None,
            requested_threads: threads,
            active_threads: None,
            applied: false,
            locked: false,
            reason: None,
        })
    })
}

fn snapshot(state: &PerformanceState) -> PerformanceSettings {
    PerformanceSettings {
        requested_profile: state.requested_profile,
        active_profile: state.active_profile,
        requested_threads: state.requested_threads,
        active_threads: state.active_threads,
        applied: state.applied,
        locked: state.locked,
        reason: state.reason.clone(),
    }
}

pub fn get_performance_settings() -> Result<PerformanceSettings, String> {
    performance_state()
        .lock()
        .map(|state| snapshot(&state))
        .map_err(|_| "La configuración de rendimiento quedó bloqueada.".to_owned())
}

pub fn set_performance_profile(profile: PerformanceProfile) -> Result<PerformanceSettings, String> {
    let requested_threads = profile_thread_count(profile, logical_cpu_count());
    let mut state = performance_state()
        .lock()
        .map_err(|_| "La configuración de rendimiento quedó bloqueada.".to_owned())?;

    state.requested_profile = profile;
    state.requested_threads = requested_threads;

    if state.locked {
        return Ok(snapshot(&state));
    }

    match ThreadPoolBuilder::new()
        .num_threads(requested_threads)
        .build_global()
    {
        Ok(()) => {
            state.active_profile = Some(profile);
            state.active_threads = Some(requested_threads);
            state.applied = true;
            state.reason = None;
        }
        Err(error) => {
            // Rayon exposes one global pool. Once another native operation
            // initializes it, changing its size would be misleading.
            state.applied = false;
            state.locked = true;
            state.reason = Some(format!(
                "El motor de concurrencia ya está inicializado; este perfil quedará para el próximo arranque ({error})."
            ));
        }
    }

    Ok(snapshot(&state))
}

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

pub(crate) fn available_memory_bytes() -> Option<u64> {
    let mut system = system_snapshot().lock().ok()?;
    system.refresh_memory();
    Some(system.available_memory())
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

    #[test]
    fn performance_profiles_resolve_to_bounded_thread_counts() {
        assert_eq!(
            profile_thread_count(PerformanceProfile::Conservative, 12),
            1
        );
        assert_eq!(profile_thread_count(PerformanceProfile::Balanced, 12), 6);
        assert_eq!(profile_thread_count(PerformanceProfile::Balanced, 5), 3);
        assert_eq!(profile_thread_count(PerformanceProfile::Maximum, 12), 12);
        assert_eq!(
            profile_thread_count(PerformanceProfile::Maximum, 128),
            MAX_THREADS
        );
    }

    #[test]
    fn performance_profiles_always_leave_at_least_one_thread() {
        for profile in [
            PerformanceProfile::Conservative,
            PerformanceProfile::Balanced,
            PerformanceProfile::Maximum,
        ] {
            assert_eq!(profile_thread_count(profile, 0), 1);
        }
    }
}
