use std::time::{Duration, Instant};
use rcm_core::DaemonState;

#[derive(Debug, Clone)]
pub struct CrashLoopBreaker {
    max_crashes: usize,
    window: Duration,
    crash_timestamps: Vec<Instant>,
}

impl CrashLoopBreaker {
    pub fn new(max_crashes: usize, window: Duration) -> Self {
        Self {
            max_crashes,
            window,
            crash_timestamps: Vec::new(),
        }
    }

    /// Records a crash event. Returns true if retry is allowed, false if breaker tripped (DM-4)
    pub fn record_crash(&mut self, now: Instant) -> bool {
        // Prune timestamps older than window
        self.crash_timestamps
            .retain(|&t| now.duration_since(t) <= self.window);

        self.crash_timestamps.push(now);

        self.crash_timestamps.len() < self.max_crashes
    }

    pub fn reset(&mut self) {
        self.crash_timestamps.clear();
    }
}

pub struct HealthMonitor {
    breaker: CrashLoopBreaker,
    current_state: DaemonState,
    last_execute_id: Option<String>,
    crash_count: u32,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            breaker: CrashLoopBreaker::new(5, Duration::from_secs(120)),
            current_state: DaemonState::Stopped,
            last_execute_id: None,
            crash_count: 0,
        }
    }

    pub fn current_state(&self) -> &DaemonState {
        &self.current_state
    }

    pub fn set_state(&mut self, state: DaemonState) {
        if state.is_ready() {
            self.crash_count = 0;
            self.breaker.reset();
        }
        self.current_state = state;
    }

    pub fn compute_backoff(&self) -> Duration {
        let secs = (1u64 << self.crash_count.min(6)).min(60);
        Duration::from_secs(secs)
    }

    pub fn on_crash(&mut self, now: Instant, exit_code: Option<i32>, last_err: String) -> DaemonState {
        self.crash_count += 1;
        let allowed = self.breaker.record_crash(now);
        if allowed {
            self.current_state = DaemonState::Crashed {
                exit_code,
                last_error: last_err,
            };
        } else {
            self.current_state = DaemonState::Failed {
                reason: format!(
                    "Crash loop detected (5 crashes within 2 minutes). Last error: {}",
                    last_err
                ),
            };
        }
        self.current_state.clone()
    }

    pub fn check_execute_id(&mut self, new_id: &str) -> bool {
        match &self.last_execute_id {
            Some(old) if old != new_id => {
                self.last_execute_id = Some(new_id.to_string());
                true // changed!
            }
            None => {
                self.last_execute_id = Some(new_id.to_string());
                false
            }
            _ => false,
        }
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}
