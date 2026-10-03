use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Default polling interval when watch coverage is degraded/unavailable (2 seconds per spec).
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Maximum backoff interval on repeated poll failures (30 seconds).
pub const MAX_POLL_BACKOFF: Duration = Duration::from_secs(30);

/// Freshness state for a watched path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathFreshness {
    pub path: PathBuf,
    pub is_degraded: bool,
    pub consecutive_failures: u32,
    pub last_reconciled: Option<Instant>,
}

impl PathFreshness {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            is_degraded: false,
            consecutive_failures: 0,
            last_reconciled: None,
        }
    }

    /// Next recommended poll duration given failure backoff.
    pub fn next_poll_delay(&self) -> Duration {
        if self.consecutive_failures == 0 {
            DEFAULT_POLL_INTERVAL
        } else {
            let mult = 2u64.saturating_pow(self.consecutive_failures.min(5));
            (DEFAULT_POLL_INTERVAL * (mult as u32)).min(MAX_POLL_BACKOFF)
        }
    }
}

/// Tracks degradation and reconciliation state across watched roots.
pub struct ReconciliationManager {
    states: RwLock<HashMap<PathBuf, PathFreshness>>,
}

impl ReconciliationManager {
    pub fn new() -> Self {
        Self {
            states: RwLock::new(HashMap::new()),
        }
    }

    /// Mark a path as degraded (e.g., watcher overflow, dropped events, or unsupported backend).
    pub fn mark_degraded(&self, path: &Path) {
        let mut states = self.states.write().unwrap();
        let entry = states
            .entry(path.to_path_buf())
            .or_insert_with(|| PathFreshness::new(path.to_path_buf()));
        entry.is_degraded = true;
    }

    /// Mark a path as successfully reconciled / fresh.
    pub fn mark_reconciled(&self, path: &Path, now: Instant) {
        let mut states = self.states.write().unwrap();
        let entry = states
            .entry(path.to_path_buf())
            .or_insert_with(|| PathFreshness::new(path.to_path_buf()));
        entry.is_degraded = false;
        entry.consecutive_failures = 0;
        entry.last_reconciled = Some(now);
    }

    /// Record a reconciliation failure for backoff calculation.
    pub fn record_failure(&self, path: &Path) {
        let mut states = self.states.write().unwrap();
        let entry = states
            .entry(path.to_path_buf())
            .or_insert_with(|| PathFreshness::new(path.to_path_buf()));
        entry.is_degraded = true;
        entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
    }

    /// Check if a path is currently degraded.
    pub fn is_degraded(&self, path: &Path) -> bool {
        let states = self.states.read().unwrap();
        states.get(path).map(|s| s.is_degraded).unwrap_or(false)
    }

    /// Get current freshness details for a path.
    pub fn get_freshness(&self, path: &Path) -> Option<PathFreshness> {
        let states = self.states.read().unwrap();
        states.get(path).cloned()
    }

    /// Untrack path upon unsubscribe.
    pub fn remove(&self, path: &Path) {
        let mut states = self.states.write().unwrap();
        states.remove(path);
    }
}

impl Default for ReconciliationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconciliation_lifecycle_and_backoff() {
        let manager = ReconciliationManager::new();
        let path = PathBuf::from(r"C:\test\folder");

        assert!(!manager.is_degraded(&path));

        // Mark degraded
        manager.mark_degraded(&path);
        assert!(manager.is_degraded(&path));

        let f = manager.get_freshness(&path).unwrap();
        assert_eq!(f.next_poll_delay(), DEFAULT_POLL_INTERVAL);

        // Record failure
        manager.record_failure(&path);
        let f2 = manager.get_freshness(&path).unwrap();
        assert_eq!(f2.consecutive_failures, 1);
        assert_eq!(f2.next_poll_delay(), Duration::from_secs(4));

        // Mark reconciled
        manager.mark_reconciled(&path, Instant::now());
        assert!(!manager.is_degraded(&path));
        let f3 = manager.get_freshness(&path).unwrap();
        assert_eq!(f3.consecutive_failures, 0);
        assert_eq!(f3.next_poll_delay(), DEFAULT_POLL_INTERVAL);
    }
}
