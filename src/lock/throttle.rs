use std::time::Duration;

const FREE_ATTEMPTS: u32 = 3;
const STEPS_SECONDS: [u64; 5] = [15, 30, 60, 180, 300];

pub fn cooldown_after(failures: u32) -> Duration {
    if failures < FREE_ATTEMPTS {
        return Duration::ZERO;
    }
    let step = usize::try_from(failures - FREE_ATTEMPTS).unwrap_or(usize::MAX);
    let seconds = STEPS_SECONDS
        .get(step)
        .or(STEPS_SECONDS.last())
        .copied()
        .unwrap_or_default();
    Duration::from_secs(seconds)
}

#[cfg(test)]
mod tests {
    use super::cooldown_after;
    use std::time::Duration;

    #[test]
    fn allows_a_few_typos_for_free() {
        assert_eq!(cooldown_after(0), Duration::ZERO);
        assert_eq!(cooldown_after(2), Duration::ZERO);
    }

    #[test]
    fn grows_then_caps() {
        assert_eq!(cooldown_after(3), Duration::from_secs(15));
        assert_eq!(cooldown_after(4), Duration::from_secs(30));
        assert_eq!(cooldown_after(7), Duration::from_secs(300));
        assert_eq!(cooldown_after(500), Duration::from_secs(300));
    }
}
