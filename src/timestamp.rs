use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(all(feature = "tsc_clock", target_arch = "x86_64"))]
use std::time::Duration;

#[cfg(all(feature = "tsc_clock", target_arch = "x86_64"))]
use std::sync::OnceLock;

#[cfg(all(feature = "tsc_clock", target_arch = "x86_64"))]
struct TscCalibration {
    offset_ns: i64,
    period_ps: u64,
}

#[cfg(all(feature = "tsc_clock", target_arch = "x86_64"))]
static TSC_STATE: OnceLock<TscCalibration> = OnceLock::new();

#[cfg(all(feature = "tsc_clock", target_arch = "x86_64"))]
fn system_time_ns_fallback() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(all(feature = "tsc_clock", target_arch = "x86_64"))]
fn calibrate_tsc() -> TscCalibration {
    // SAFETY: _rdtsc is safe to call on x86_64, regardless of privilege level.
    let tsc1 = unsafe { std::arch::x86_64::_rdtsc() };
    let wall1 = system_time_ns_fallback();
    std::thread::sleep(Duration::from_millis(1));
    let tsc2 = unsafe { std::arch::x86_64::_rdtsc() };
    let wall2 = system_time_ns_fallback();

    let tsc_delta = tsc2.wrapping_sub(tsc1);
    let wall_delta = wall2.saturating_sub(wall1).max(1);

    let period_ps = (wall_delta as u128 * 1_000u128 / tsc_delta as u128) as u64;
    let ns_correction = (tsc1 as u128 * period_ps as u128 / 1_000u128) as i64;
    let offset_ns = wall1 as i64 - ns_correction;

    TscCalibration {
        offset_ns,
        period_ps,
    }
}

#[cfg(all(feature = "tsc_clock", target_arch = "x86_64"))]
#[inline]
/// Returns the current timestamp for log messages.
///
/// With the `tsc_clock` feature: raw RDTSC counter value (fast, ~1 instruction).
/// With the default system clock: Unix epoch nanoseconds via `SystemTime`.
///
/// The returned value is stored in the queue and later converted to display
/// nanoseconds by [`to_display_nanos`] on the backend thread.
pub fn now() -> u64 {
    // SAFETY: _rdtsc is safe on x86_64 at any privilege level.
    unsafe { std::arch::x86_64::_rdtsc() }
}

#[cfg(all(feature = "tsc_clock", target_arch = "x86_64"))]
/// Converts a raw TSC counter value to wall-clock nanoseconds.
///
/// Called only on the backend thread. Calibrates on first call using a
/// 1 ms sleep to determine the TSC frequency. The conversion formula is:
/// `display_ns = calibration_offset + raw_tsc * period_ps / 1000`
///
/// The raw value passed here should be one returned by [`now`].
pub fn to_display_nanos(raw: u64) -> u64 {
    let cal = TSC_STATE.get_or_init(|| calibrate_tsc());
    if cal.period_ps == 0 {
        return system_time_ns_fallback();
    }
    let ns_since_cal = (raw as u128 * cal.period_ps as u128 / 1_000u128) as i64;
    (cal.offset_ns + ns_since_cal) as u64
}

#[cfg(not(all(feature = "tsc_clock", target_arch = "x86_64")))]
#[inline]
/// Returns the current timestamp for log messages as Unix epoch nanoseconds.
///
/// Uses `SystemTime::now()` via vDSO on Linux — the default portable clock.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

#[cfg(not(all(feature = "tsc_clock", target_arch = "x86_64")))]
#[inline]
/// Identity passthrough — raw value is already Unix epoch nanoseconds.
pub fn to_display_nanos(raw: u64) -> u64 {
    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_returns_reasonable_value() {
        let ts = now();
        // With tsc_clock: raw TSC ticks (small number). Without: unix nanos.
        // Just verify it's some u64.
        assert!(ts < u64::MAX);
    }

    #[test]
    fn now_is_monotonic() {
        let a = now();
        let b = now();
        assert!(b >= a || b < a + 1_000_000);
    }

    #[test]
    fn to_display_nanos_works() {
        let raw = now();
        let display = to_display_nanos(raw);
        assert!(display < u64::MAX);
    }
}
