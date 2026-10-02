//! Advisory-locked pending decision queue for System One medium-tier incidents.

use rustix::fs::{flock, FlockOperation};
use sentry_core::models::{DecisionTier, FaultClass, RemediationAction};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A pending incident staged for operator review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingIncident {
    /// Unique incident ID.
    pub incident_id: String,
    /// Failing systemd service unit name.
    pub unit: String,
    /// UTC timestamp string.
    pub timestamp: String,
    /// Fault class determined by System One classifier.
    pub fault_class: FaultClass,
    /// Calibrated confidence score.
    pub confidence: f32,
    /// Decision tier (typically Medium).
    pub tier: DecisionTier,
    /// Proposed remediation action to take if approved.
    pub proposed_action: RemediationAction,
    /// Rationale and explanation for operator.
    pub explanation: String,
    /// Relevant journal log lines.
    pub journal_excerpt: Vec<String>,
}

/// Queue manager for staging incidents under /run/syntrop/pending.
#[derive(Debug, Clone)]
pub struct PendingQueue {
    base_dir: PathBuf,
}

impl Default for PendingQueue {
    fn default() -> Self {
        let base = std::env::var("RUNTIME_DIRECTORY")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/run/syntrop"));
        Self::new(base)
    }
}

impl PendingQueue {
    /// Creates a new PendingQueue rooted at base_dir.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn pending_dir(&self) -> PathBuf {
        self.base_dir.join("pending")
    }

    fn count_file(&self) -> PathBuf {
        self.base_dir.join("pending_count")
    }

    fn lock_file_path(&self) -> PathBuf {
        self.base_dir.join("pending.lock")
    }

    /// Acquires advisory file lock, performs closure, updates pending_count, unlocks.
    fn with_lock<T, F: FnOnce(&Path, &Path) -> std::io::Result<T>>(&self, f: F) -> std::io::Result<T> {
        let pending = self.pending_dir();
        fs::create_dir_all(&pending)?;
        let lock_path = self.lock_file_path();
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        flock(&lock_file, FlockOperation::LockExclusive)
            .map_err(std::io::Error::other)?;

        let res = f(&pending, &self.count_file());

        let _ = flock(&lock_file, FlockOperation::Unlock);
        res
    }

    /// Helper to update pending_count file atomically.
    fn update_count(pending_dir: &Path, count_path: &Path) -> std::io::Result<usize> {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(pending_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|ext| ext == "json") {
                    count += 1;
                }
            }
        }
        let tmp_count = count_path.with_extension("tmp");
        {
            let mut f = File::create(&tmp_count)?;
            writeln!(f, "{}", count)?;
            f.sync_all()?;
        }
        let _ = fs::rename(tmp_count, count_path);
        Ok(count)
    }

    /// Stages an incident into the pending queue.
    pub async fn stage(&self, incident: PendingIncident) -> std::io::Result<usize> {
        let q = self.clone();
        tokio::task::spawn_blocking(move || {
            q.with_lock(|dir, count_file| {
                let file_path = dir.join(format!("{}.json", incident.incident_id));
                let json = serde_json::to_string_pretty(&incident)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                fs::write(file_path, json)?;
                Self::update_count(dir, count_file)
            })
        })
        .await
        .map_err(std::io::Error::other)?
    }

    /// Removes an incident from the pending queue by ID.
    pub async fn remove(&self, incident_id: &str) -> std::io::Result<Option<PendingIncident>> {
        let q = self.clone();
        let id_owned = incident_id.to_string();
        tokio::task::spawn_blocking(move || {
            q.with_lock(|dir, count_file| {
                let file_path = dir.join(format!("{id_owned}.json"));
                if !file_path.exists() {
                    return Ok(None);
                }
                let content = fs::read_to_string(&file_path)?;
                let inc: PendingIncident = serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                let _ = fs::remove_file(file_path);
                let _ = Self::update_count(dir, count_file);
                Ok(Some(inc))
            })
        })
        .await
        .map_err(std::io::Error::other)?
    }

    /// Lists all pending incidents currently staged.
    pub async fn list(&self) -> std::io::Result<Vec<PendingIncident>> {
        let q = self.clone();
        tokio::task::spawn_blocking(move || {
            q.with_lock(|dir, count_file| {
                let mut incidents = Vec::new();
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "json") {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(inc) = serde_json::from_str::<PendingIncident>(&content) {
                                    incidents.push(inc);
                                }
                            }
                        }
                    }
                }
                let _ = Self::update_count(dir, count_file);
                Ok(incidents)
            })
        })
        .await
        .map_err(std::io::Error::other)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_pending_queue_stage_list_remove() {
        let dir = tempdir().unwrap();
        let queue = PendingQueue::new(dir.path());

        let incident = PendingIncident {
            incident_id: "inc-1".into(),
            unit: "nginx.service".into(),
            timestamp: "2026-09-30T12:00:00Z".into(),
            fault_class: FaultClass::TransientRestart,
            confidence: 0.75,
            tier: DecisionTier::Medium,
            proposed_action: RemediationAction::Restart,
            explanation: "Medium confidence restart".into(),
            journal_excerpt: vec!["exit code 1".into()],
        };

        let count = queue.stage(incident.clone()).await.unwrap();
        assert_eq!(count, 1);

        let list = queue.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].incident_id, "inc-1");

        let removed = queue.remove("inc-1").await.unwrap();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().unit, "nginx.service");

        let list_empty = queue.list().await.unwrap();
        assert_eq!(list_empty.len(), 0);
    }
}
