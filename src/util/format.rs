//! Human-friendly formatting helpers.

use chrono::{DateTime, Local, TimeZone};

/// Format a byte count using binary units (KiB = 1024 bytes), shown as KB/MB/GB for readability.
pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

/// Format a count with thousands separators: 1234567 -> "1,234,567".
pub fn count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Format a unix timestamp (seconds) as a local date and time.
pub fn date(unix_secs: i64) -> String {
    if unix_secs <= 0 {
        return "—".to_owned();
    }
    match Local.timestamp_opt(unix_secs, 0).single() {
        Some(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        None => "—".to_owned(),
    }
}

/// Format a unix timestamp as a date only.
pub fn date_short(unix_secs: i64) -> String {
    if unix_secs <= 0 {
        return "—".to_owned();
    }
    match Local.timestamp_opt(unix_secs, 0).single() {
        Some(dt) => dt.format("%Y-%m-%d").to_string(),
        None => "—".to_owned(),
    }
}

/// "3 days ago", "2 years ago" style relative age.
pub fn age(unix_secs: i64) -> String {
    if unix_secs <= 0 {
        return "—".to_owned();
    }
    let then: DateTime<Local> = match Local.timestamp_opt(unix_secs, 0).single() {
        Some(dt) => dt,
        None => return "—".to_owned(),
    };
    let days = (Local::now() - then).num_days();
    match days {
        d if d < 1 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        d if d < 31 => format!("{d} days ago"),
        d if d < 365 => format!("{} months ago", d / 30),
        d => {
            let y = d / 365;
            if y == 1 { "1 year ago".to_owned() } else { format!("{y} years ago") }
        }
    }
}

/// Format a duration in seconds as "1m 23s".
pub fn duration(secs: f64) -> String {
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let s = secs as u64;
        if s < 3600 {
            format!("{}m {:02}s", s / 60, s % 60)
        } else {
            format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
        }
    }
}

/// Percentage of `part` in `whole`, safe for zero.
pub fn percent(part: u64, whole: u64) -> f32 {
    if whole == 0 { 0.0 } else { (part as f64 / whole as f64 * 100.0) as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1023), "1023 B");
        assert_eq!(size(1024), "1.00 KB");
        assert_eq!(size(1536), "1.50 KB");
        assert_eq!(size(10 * 1024 * 1024), "10.0 MB");
        assert_eq!(size(500 * 1024 * 1024 * 1024), "500 GB");
    }

    #[test]
    fn counts() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1000), "1,000");
        assert_eq!(count(1234567), "1,234,567");
    }

    #[test]
    fn percents() {
        assert_eq!(percent(0, 0), 0.0);
        assert_eq!(percent(50, 200), 25.0);
    }
}
