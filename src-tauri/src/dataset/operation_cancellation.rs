use super::*;

#[derive(Clone)]
pub(super) struct PrepareCancellation {
    app: Option<AppHandle>,
    generation: u64,
    #[cfg(test)]
    cancelled_for_test: Option<std::sync::Arc<AtomicBool>>,
}

#[derive(Clone)]
pub(super) struct DatasetComparisonCancellation {
    app: AppHandle,
    generation: u64,
}

impl DatasetComparisonCancellation {
    pub(super) fn begin(app: &AppHandle) -> Self {
        let generation = app.state::<DatasetState>().begin_dataset_comparison();
        Self {
            app: app.clone(),
            generation,
        }
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.app
            .state::<DatasetState>()
            .dataset_comparison_was_cancelled(self.generation)
    }

    pub(super) fn ensure(&self) -> Result<(), String> {
        ensure_not_cancelled(self.is_cancelled())
    }

    pub(super) fn callback(&self) -> impl Fn() -> bool + Clone + Send + Sync + 'static {
        let cancellation = self.clone();
        move || cancellation.is_cancelled()
    }

    pub(super) fn commit<T>(
        &self,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let state = self.app.state::<DatasetState>();
        let _guard = state
            .dataset_comparison_commit_lock
            .lock()
            .map_err(|_| "La publicación de la comparación quedó bloqueada.".to_owned())?;
        self.ensure()?;
        operation()
    }
}

#[derive(Clone)]
pub(super) struct QualityValidationCancellation {
    app: AppHandle,
    generation: u64,
}

impl QualityValidationCancellation {
    pub(super) fn begin(app: &AppHandle) -> Self {
        let generation = app.state::<DatasetState>().begin_quality_validation();
        Self {
            app: app.clone(),
            generation,
        }
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.app
            .state::<DatasetState>()
            .quality_validation_was_cancelled(self.generation)
    }

    pub(super) fn ensure(&self) -> Result<(), String> {
        ensure_not_cancelled(self.is_cancelled())
    }

    pub(super) fn callback(&self) -> impl Fn() -> bool + Clone + Send + Sync + 'static {
        let cancellation = self.clone();
        move || cancellation.is_cancelled()
    }
}

#[derive(Clone)]
pub(super) struct DatabasePreflightCancellation {
    app: AppHandle,
    generation: u64,
}

impl DatabasePreflightCancellation {
    pub(super) fn begin(app: &AppHandle) -> Self {
        let generation = app.state::<DatasetState>().begin_database_preflight();
        Self {
            app: app.clone(),
            generation,
        }
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.app
            .state::<DatasetState>()
            .database_preflight_was_cancelled(self.generation)
    }

    pub(super) fn ensure(&self) -> Result<(), String> {
        ensure_not_cancelled(self.is_cancelled())
    }

    pub(super) fn callback(&self) -> impl Fn() -> bool + Clone + Send + Sync + 'static {
        let cancellation = self.clone();
        move || cancellation.is_cancelled()
    }
}

impl PrepareCancellation {
    pub(super) fn begin(app: &AppHandle) -> Self {
        let generation = app.state::<DatasetState>().begin_prepare();
        Self {
            app: Some(app.clone()),
            generation,
            #[cfg(test)]
            cancelled_for_test: None,
        }
    }

    #[cfg(test)]
    pub(super) fn disabled() -> Self {
        Self {
            app: None,
            generation: 0,
            cancelled_for_test: None,
        }
    }

    #[cfg(test)]
    pub(super) fn cancelled_for_test() -> Self {
        Self {
            app: None,
            generation: 0,
            cancelled_for_test: Some(std::sync::Arc::new(AtomicBool::new(true))),
        }
    }

    pub(super) fn is_cancelled(&self) -> bool {
        #[cfg(test)]
        if self
            .cancelled_for_test
            .as_ref()
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
        {
            return true;
        }
        self.app.as_ref().is_some_and(|app| {
            app.state::<DatasetState>()
                .prepare_was_cancelled(self.generation)
        })
    }

    pub(super) fn ensure(&self) -> Result<(), String> {
        ensure_not_cancelled(self.is_cancelled())
    }

    pub(super) fn commit<T>(
        &self,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if let Some(app) = &self.app {
            let state = app.state::<DatasetState>();
            let _guard = state
                .prepare_commit_lock
                .lock()
                .map_err(|_| "La publicación de la preparación quedó bloqueada.".to_owned())?;
            self.ensure()?;
            operation()
        } else {
            self.ensure()?;
            operation()
        }
    }

    pub(super) fn callback(&self) -> impl Fn() -> bool + Send + 'static {
        let cancellation = self.clone();
        move || cancellation.is_cancelled()
    }
}

pub(super) struct ReviewMutationGuard {
    app: AppHandle,
}

impl Drop for ReviewMutationGuard {
    fn drop(&mut self) {
        self.app
            .state::<DatasetState>()
            .review_mutation_in_flight
            .store(false, Ordering::Release);
    }
}

#[derive(Clone)]
pub(super) struct ReviewMutationCancellation {
    app: Option<AppHandle>,
    generation: u64,
    #[cfg(test)]
    cancelled_for_test: Option<std::sync::Arc<AtomicBool>>,
    #[cfg(test)]
    cancel_at_commit_for_test: bool,
}

impl ReviewMutationCancellation {
    pub(super) fn begin(app: &AppHandle) -> Result<(ReviewMutationGuard, Self), String> {
        let state = app.state::<DatasetState>();
        let generation = state.begin_review_mutation()?;
        Ok((
            ReviewMutationGuard { app: app.clone() },
            Self {
                app: Some(app.clone()),
                generation,
                #[cfg(test)]
                cancelled_for_test: None,
                #[cfg(test)]
                cancel_at_commit_for_test: false,
            },
        ))
    }

    #[cfg(test)]
    pub(super) fn disabled() -> Self {
        Self {
            app: None,
            generation: 0,
            cancelled_for_test: None,
            cancel_at_commit_for_test: false,
        }
    }

    #[cfg(test)]
    pub(super) fn cancel_at_commit_for_test() -> Self {
        Self {
            app: None,
            generation: 0,
            cancelled_for_test: Some(std::sync::Arc::new(AtomicBool::new(false))),
            cancel_at_commit_for_test: true,
        }
    }

    pub(super) fn is_cancelled(&self) -> bool {
        #[cfg(test)]
        if self
            .cancelled_for_test
            .as_ref()
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
        {
            return true;
        }
        self.app.as_ref().is_some_and(|app| {
            app.state::<DatasetState>()
                .review_mutation_was_cancelled(self.generation)
        })
    }

    pub(super) fn ensure(&self) -> Result<(), String> {
        ensure_not_cancelled(self.is_cancelled())
    }

    pub(super) fn commit<T>(
        &self,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if let Some(app) = &self.app {
            let state = app.state::<DatasetState>();
            let _guard = state
                .review_mutation_commit_lock
                .lock()
                .map_err(|_| "La publicación de Review quedó bloqueada.".to_owned())?;
            #[cfg(test)]
            self.cancel_test_operation_at_commit();
            self.ensure()?;
            operation()
        } else {
            #[cfg(test)]
            self.cancel_test_operation_at_commit();
            self.ensure()?;
            operation()
        }
    }

    #[cfg(test)]
    fn cancel_test_operation_at_commit(&self) {
        if self.cancel_at_commit_for_test {
            if let Some(cancelled) = &self.cancelled_for_test {
                cancelled.store(true, Ordering::Release);
            }
        }
    }

    pub(super) fn callback(&self) -> impl Fn() -> bool + Clone + Send + 'static {
        let cancellation = self.clone();
        move || cancellation.is_cancelled()
    }
}
