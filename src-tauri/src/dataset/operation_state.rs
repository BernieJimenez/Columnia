use super::*;
use crate::crash_report::LockRecovering;

impl DatasetState {
    pub(crate) fn queue_dropped_path(&self, path: PathBuf) {
        if let Ok(mut pending) = self.pending_drop.lock() {
            *pending = Some(path);
        }
    }

    pub(crate) fn take_dropped_path(&self) -> Result<Option<PathBuf>, String> {
        Ok(self.pending_drop.lock_recovering().take())
    }

    pub(super) fn begin_load(&self) -> Result<u64, String> {
        let _guard = self.load_commit_lock.lock_recovering();
        Ok(self
            .load_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1))
    }

    pub(super) fn commit_load<T>(
        &self,
        generation: u64,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _guard = self.load_commit_lock.lock_recovering();
        ensure_not_cancelled(self.load_was_cancelled(generation))?;
        operation()
    }

    pub(super) fn begin_profile(&self) -> u64 {
        self.profile_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_temporal(&self) -> u64 {
        self.temporal_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_export(&self) -> u64 {
        self.export_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_query(&self) -> u64 {
        self.query_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_dataset_page(&self) -> u64 {
        self.dataset_page_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_snapshot_comparison(&self) -> u64 {
        self.snapshot_comparison_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_dataset_comparison(&self) -> u64 {
        self.dataset_comparison_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_quality_validation(&self) -> u64 {
        self.quality_validation_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_database_preflight(&self) -> u64 {
        self.database_preflight_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(crate) fn begin_database_connection(&self) -> u64 {
        self.database_connection_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_prepare(&self) -> u64 {
        self.prepare_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1)
    }

    pub(super) fn begin_review_mutation(&self) -> Result<u64, String> {
        self.review_mutation_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Ya hay una operación de Review en curso.".to_owned())?;
        Ok(self
            .review_mutation_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1))
    }

    pub(super) fn load_was_cancelled(&self, generation: u64) -> bool {
        self.load_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn profile_was_cancelled(&self, generation: u64) -> bool {
        self.profile_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn temporal_was_cancelled(&self, generation: u64) -> bool {
        self.temporal_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn export_was_cancelled(&self, generation: u64) -> bool {
        self.export_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn query_was_cancelled(&self, generation: u64) -> bool {
        self.query_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn dataset_page_was_cancelled(&self, generation: u64) -> bool {
        self.dataset_page_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn snapshot_comparison_was_cancelled(&self, generation: u64) -> bool {
        self.snapshot_comparison_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn dataset_comparison_was_cancelled(&self, generation: u64) -> bool {
        self.dataset_comparison_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn quality_validation_was_cancelled(&self, generation: u64) -> bool {
        self.quality_validation_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn database_preflight_was_cancelled(&self, generation: u64) -> bool {
        self.database_preflight_generation.load(Ordering::SeqCst) != generation
    }

    pub(crate) fn database_connection_was_cancelled(&self, generation: u64) -> bool {
        self.database_connection_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn prepare_was_cancelled(&self, generation: u64) -> bool {
        self.prepare_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn review_mutation_was_cancelled(&self, generation: u64) -> bool {
        self.review_mutation_generation.load(Ordering::SeqCst) != generation
    }

    pub(super) fn remember_last_export(&self, path: PathBuf) {
        if let Ok(mut last_export_path) = self.last_export_path.lock() {
            *last_export_path = Some(path);
        }
    }

    pub(super) fn last_export(&self) -> Result<PathBuf, String> {
        self.last_export_path
            .lock_recovering()
            .clone()
            .ok_or_else(|| "Todavía no hay una exportación disponible para abrir.".to_owned())
    }

    pub(super) fn cancel(&self, operation: &str) -> Result<(), String> {
        if operation == "load" {
            let _guard = self.load_commit_lock.lock_recovering();
            self.load_generation.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "projectCatalog" {
            self.project_catalog_generation
                .fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "projectVersions" {
            self.project_versions_generation
                .fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "projectDelete" {
            let _guard = self.project_delete_commit_lock.lock_recovering();
            self.project_delete_generation
                .fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "projectSave" {
            let _guard = self.project_save_commit_lock.lock_recovering();
            self.project_save_generation.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "projectOpen" {
            let _guard = self.project_open_commit_lock.lock_recovering();
            self.project_open_generation.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "prepare" {
            let _guard = self.prepare_commit_lock.lock_recovering();
            self.prepare_generation.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "reviewMutation" {
            let _guard = self.review_mutation_commit_lock.lock_recovering();
            self.review_mutation_generation
                .fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "datasetComparison" {
            let _guard = self.dataset_comparison_commit_lock.lock_recovering();
            self.dataset_comparison_generation
                .fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "qualityValidation" {
            self.quality_validation_generation
                .fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "databasePreflight" {
            self.database_preflight_generation
                .fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "databaseConnection" {
            self.database_connection_generation
                .fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        if operation == "datasetPage" {
            self.dataset_page_generation.fetch_add(1, Ordering::SeqCst);
            return Ok(());
        }
        let generation = match operation {
            "profile" => &self.profile_generation,
            "temporal" => &self.temporal_generation,
            "export" => &self.export_generation,
            "query" => &self.query_generation,
            "snapshotComparison" => &self.snapshot_comparison_generation,
            _ => return Err("La operación indicada no admite cancelación.".to_owned()),
        };
        generation.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
