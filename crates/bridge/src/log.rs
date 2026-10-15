//! Minimales Logging auf stderr: `2026-10-15T12:00:00Z LEVEL nachricht`.

use opengdt_worklist::time;

fn stamp() -> String {
    let (y, mo, d, h, mi, s) = time::civil(time::now_unix());
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

pub fn line(level: &str, msg: &str) {
    eprintln!("{} {level:<5} {msg}", stamp());
}

#[macro_export]
macro_rules! info { ($($a:tt)*) => { $crate::log::line("INFO", &format!($($a)*)) } }
#[macro_export]
macro_rules! warn { ($($a:tt)*) => { $crate::log::line("WARN", &format!($($a)*)) } }
#[macro_export]
macro_rules! error { ($($a:tt)*) => { $crate::log::line("ERROR", &format!($($a)*)) } }
