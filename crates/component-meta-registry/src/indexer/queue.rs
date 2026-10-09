use std::time::Duration;

use tokio::time::Instant;
use tracing::{error, info, warn};
use wasm_package_manager::manager::{Manager, TaskOutcome};

use super::{Indexer, LEADER_RETRY_INTERVAL};

impl Indexer {
    pub(super) async fn process_queue_step(&mut self) -> bool {
        self.process_queue_step_with(Manager::process_next_task)
            .await
    }

    pub(super) async fn process_queue_step_with(
        &mut self,
        process: impl AsyncFnOnce(&Manager) -> anyhow::Result<TaskOutcome>,
    ) -> bool {
        if Instant::now() < self.queue_paused_until || !self.check_leadership().await {
            return false;
        }
        if Instant::now() >= self.next_recovery {
            self.next_recovery = Instant::now() + LEADER_RETRY_INTERVAL;
            match self.manager.recover_in_progress_tasks().await {
                Ok(0) => {}
                Ok(n) => info!(count = n, "Re-queued tasks abandoned by a previous worker"),
                Err(e) => warn!(error = %e, "Failed to recover in-progress tasks"),
            }
        }
        match process(&self.manager).await {
            Ok(TaskOutcome::Succeeded) => {
                self.queue_errors = 0;
                tokio::time::sleep(Duration::from_millis(250)).await;
                true
            }
            Ok(TaskOutcome::Failed) => {
                self.queue_errors = 0;
                tokio::time::sleep(Duration::from_secs(2)).await;
                true
            }
            Ok(TaskOutcome::Empty) => {
                self.queue_errors = 0;
                false
            }
            Err(e) => {
                self.queue_errors += 1;
                error!(error = %e, "Error processing fetch queue");
                if self.queue_errors >= 5 {
                    error!("Too many consecutive queue errors, pausing queue processing");
                    self.queue_errors = 0;
                    self.queue_paused_until = Instant::now() + LEADER_RETRY_INTERVAL;
                    return false;
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
                true
            }
        }
    }
}
