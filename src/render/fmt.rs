pub fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn format_duration(minutes: u64) -> String {
    if minutes == 0 {
        return "<1m".to_string();
    }

    let days = minutes / 1440;
    let hours = (minutes % 1440) / 60;
    let mins = minutes % 60;

    if days > 0 {
        if hours > 0 {
            format!("{}d {}h", days, hours)
        } else {
            format!("{}d", days)
        }
    } else if hours > 0 {
        if mins > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}h", hours)
        }
    } else {
        format!("{}m", mins)
    }
}

pub fn format_speed(toks_per_sec: f64) -> String {
    if toks_per_sec >= 1000.0 {
        format!("↗{:.1}K/s", toks_per_sec / 1000.0)
    } else {
        format!("↗{:.0}/s", toks_per_sec)
    }
}

/// Formats a reset duration in minutes with adaptive granularity.
/// Unlike `format_duration()` which omits trailing zeros for brevity,
/// this always shows sub-units for precision:
/// - <1h: `Xm`
/// - 1h to <24h: `Xh Xm`
/// - 24h+: `Xd Xh Xm`
pub fn format_reset_duration(minutes: u64) -> String {
    if minutes == 0 {
        return "<1m".to_string();
    }

    let days = minutes / 1440;
    let hours = (minutes % 1440) / 60;
    let mins = minutes % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_zero() {
        assert_eq!(format_number(0), "0");
    }

    #[test]
    fn formats_below_thousand() {
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn formats_at_thousand_boundary() {
        assert_eq!(format_number(1000), "1.0k");
    }

    #[test]
    fn formats_with_decimal_k() {
        assert_eq!(format_number(1500), "1.5k");
    }

    #[test]
    fn formats_200k() {
        assert_eq!(format_number(200_000), "200.0k");
    }

    #[test]
    fn formats_just_below_million() {
        assert_eq!(format_number(999_999), "1000.0k");
    }

    #[test]
    fn formats_at_million_boundary() {
        assert_eq!(format_number(1_000_000), "1.0M");
    }

    #[test]
    fn formats_above_million() {
        assert_eq!(format_number(1_500_000), "1.5M");
    }

    #[test]
    fn duration_zero_minutes() {
        assert_eq!(format_duration(0), "<1m");
    }

    #[test]
    fn duration_minutes_only() {
        assert_eq!(format_duration(45), "45m");
    }

    #[test]
    fn duration_exactly_one_hour() {
        assert_eq!(format_duration(60), "1h");
    }

    #[test]
    fn duration_hours_and_minutes() {
        assert_eq!(format_duration(90), "1h 30m");
    }

    #[test]
    fn duration_exactly_one_day() {
        assert_eq!(format_duration(1440), "1d");
    }

    #[test]
    fn duration_days_and_hours() {
        assert_eq!(format_duration(1500), "1d 1h");
    }

    #[test]
    fn duration_days_only_no_leftover_hours() {
        assert_eq!(format_duration(2880), "2d");
    }

    #[test]
    fn speed_below_thousand() {
        assert_eq!(format_speed(999.0), "↗999/s");
    }

    #[test]
    fn speed_at_thousand_boundary() {
        assert_eq!(format_speed(1000.0), "↗1.0K/s");
    }

    #[test]
    fn speed_above_thousand() {
        assert_eq!(format_speed(1500.0), "↗1.5K/s");
    }

    #[test]
    fn speed_small_value() {
        assert_eq!(format_speed(42.0), "↗42/s");
    }

    // format_reset_duration tests

    #[test]
    fn reset_duration_zero() {
        assert_eq!(format_reset_duration(0), "<1m");
    }

    #[test]
    fn reset_duration_minutes_only() {
        assert_eq!(format_reset_duration(15), "15m");
        assert_eq!(format_reset_duration(59), "59m");
    }

    #[test]
    fn reset_duration_exact_hour() {
        assert_eq!(format_reset_duration(60), "1h 0m");
    }

    #[test]
    fn reset_duration_hours_and_minutes() {
        assert_eq!(format_reset_duration(90), "1h 30m");
        assert_eq!(format_reset_duration(120), "2h 0m");
        assert_eq!(format_reset_duration(1439), "23h 59m");
    }

    #[test]
    fn reset_duration_exact_day() {
        assert_eq!(format_reset_duration(1440), "1d 0h 0m");
    }

    #[test]
    fn reset_duration_days_with_hours() {
        assert_eq!(format_reset_duration(1500), "1d 1h 0m");
        assert_eq!(format_reset_duration(1530), "1d 1h 30m");
    }

    #[test]
    fn reset_duration_multiple_days() {
        assert_eq!(format_reset_duration(2880), "2d 0h 0m");
        assert_eq!(format_reset_duration(10080), "7d 0h 0m");
        assert_eq!(format_reset_duration(10140), "7d 1h 0m");
    }
}
