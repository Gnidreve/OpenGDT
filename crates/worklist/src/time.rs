//! Minimale Zeit-Hilfen auf `std`-Basis (UTC).

use std::time::{SystemTime, UNIX_EPOCH};

/// (Jahr, Monat, Tag, Stunde, Minute, Sekunde) in UTC für Unix-Sekunden.
pub fn civil(unix_secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = unix_secs.div_euclid(86_400);
    let rem = unix_secs.rem_euclid(86_400) as u32;
    // Howard Hinnant, "civil_from_days"
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60)
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `(YYYYMMDD, HHMMSS)` für Unix-Sekunden, nach Anwendung von `offset_minutes`.
pub fn date_time(unix_secs: i64, offset_minutes: i32) -> (String, String) {
    let (y, mo, d, h, mi, s) = civil(unix_secs + offset_minutes as i64 * 60);
    (
        format!("{y:04}{mo:02}{d:02}"),
        format!("{h:02}{mi:02}{s:02}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(civil(0), (1970, 1, 1, 0, 0, 0));
        // 2026-10-15 12:34:56 UTC
        assert_eq!(civil(1_792_067_696), (2026, 10, 15, 12, 34, 56));
        // Schaltjahr
        assert_eq!(civil(951_782_400).0, 2000);
        assert_eq!(date_time(951_782_400, 0).0, "20000229");
        // Offset über Mitternacht
        assert_eq!(date_time(1_792_067_696 + 12 * 3600, 60).0, "20261016");
    }
}
