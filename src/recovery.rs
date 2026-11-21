use std::time::{Duration, Instant};

const OFFLINE_GRACE: Duration = Duration::from_secs(20);
const RETRY_STEPS: [Duration; 5] = [
    Duration::from_secs(15),
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(120),
    Duration::from_secs(300),
];
const CRASH_WINDOW: Duration = Duration::from_secs(120);
const CRASH_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(9),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Wait,
    Reload,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionWatchdog {
    offline_since: Option<Instant>,
    next_retry: Option<Instant>,
    retries: usize,
}

impl ConnectionWatchdog {
    pub fn went_online(&mut self) {
        *self = Self::default();
    }

    pub fn went_offline(&mut self, now: Instant) {
        if self.offline_since.is_none() {
            self.offline_since = Some(now);
            self.next_retry = Some(now + OFFLINE_GRACE);
        }
    }

    pub fn network_returned(&mut self, now: Instant) {
        if self.offline_since.is_some() {
            self.next_retry = Some(now);
        }
    }

    pub fn retry_now(&mut self, now: Instant) {
        self.network_returned(now);
    }

    pub fn is_offline(&self) -> bool {
        self.offline_since.is_some()
    }

    pub fn countdown(&self, now: Instant) -> Option<Duration> {
        self.next_retry
            .map(|retry| retry.saturating_duration_since(now))
    }

    pub fn poll(&mut self, now: Instant) -> Verdict {
        match self.next_retry {
            Some(retry) if now >= retry => {
                let step = RETRY_STEPS[self.retries.min(RETRY_STEPS.len() - 1)];
                self.retries += 1;
                self.next_retry = Some(now + step);
                Verdict::Reload
            }
            _ => Verdict::Wait,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CrashBudget {
    crashes: Vec<Instant>,
}

impl CrashBudget {
    pub fn record(&mut self, now: Instant) -> Option<Duration> {
        self.crashes
            .retain(|crash| now.duration_since(*crash) < CRASH_WINDOW);
        let delay = CRASH_DELAYS.get(self.crashes.len()).copied();
        self.crashes.push(now);
        delay
    }

    pub fn forgive(&mut self) {
        self.crashes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionWatchdog, CrashBudget, Verdict};
    use std::time::{Duration, Instant};

    const fn seconds(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    #[test]
    fn waits_for_the_grace_period_before_reloading() {
        let start = Instant::now();
        let mut watchdog = ConnectionWatchdog::default();
        watchdog.went_offline(start);
        assert_eq!(watchdog.poll(start + seconds(10)), Verdict::Wait);
        assert_eq!(watchdog.poll(start + seconds(20)), Verdict::Reload);
        assert_eq!(watchdog.countdown(start + seconds(20)), Some(seconds(15)));
    }

    #[test]
    fn backs_off_then_caps() {
        let start = Instant::now();
        let mut watchdog = ConnectionWatchdog::default();
        watchdog.went_offline(start);
        let mut now = start + seconds(20);
        let mut gaps = Vec::new();
        for _ in 0..7 {
            assert_eq!(watchdog.poll(now), Verdict::Reload);
            let gap = watchdog.countdown(now).unwrap();
            gaps.push(gap.as_secs());
            now += gap;
        }
        assert_eq!(gaps, vec![15, 30, 60, 120, 300, 300, 300]);
    }

    #[test]
    fn reacts_at_once_when_the_network_returns() {
        let start = Instant::now();
        let mut watchdog = ConnectionWatchdog::default();
        watchdog.went_offline(start);
        watchdog.network_returned(start + seconds(2));
        assert_eq!(watchdog.poll(start + seconds(2)), Verdict::Reload);
    }

    #[test]
    fn resets_when_back_online() {
        let start = Instant::now();
        let mut watchdog = ConnectionWatchdog::default();
        watchdog.went_offline(start);
        watchdog.went_online();
        assert!(!watchdog.is_offline());
        assert_eq!(watchdog.poll(start + seconds(100)), Verdict::Wait);
        watchdog.network_returned(start);
        assert_eq!(watchdog.countdown(start), None);
    }

    #[test]
    fn gives_up_after_repeated_crashes() {
        let start = Instant::now();
        let mut budget = CrashBudget::default();
        assert_eq!(budget.record(start), Some(seconds(1)));
        assert_eq!(budget.record(start + seconds(5)), Some(seconds(3)));
        assert_eq!(budget.record(start + seconds(10)), Some(seconds(9)));
        assert_eq!(budget.record(start + seconds(20)), None);
    }

    #[test]
    fn forgets_old_crashes() {
        let start = Instant::now();
        let mut budget = CrashBudget::default();
        budget.record(start);
        budget.record(start + seconds(1));
        assert_eq!(budget.record(start + seconds(300)), Some(seconds(1)));
    }
}
