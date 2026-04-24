use anyhow::{anyhow, bail, Context, Result};
use git2::{Repository, Sort};
use std::fmt::Write;

use crate::guard::reject_flag_arg;
use crate::tools::format_git_time;

pub fn git_log(
    repo: &Repository,
    max_count: usize,
    start_timestamp: Option<&str>,
    end_timestamp: Option<&str>,
) -> Result<String> {
    let start = start_timestamp.map(parse_timestamp).transpose()?;
    let end = end_timestamp.map(parse_timestamp).transpose()?;

    let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
    revwalk.set_sorting(Sort::TIME)?;
    revwalk.push_head().context("failed to push HEAD")?;

    let mut entries: Vec<String> = Vec::new();
    for oid in revwalk {
        if entries.len() >= max_count {
            break;
        }
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let time = commit.time();
        let seconds = time.seconds();

        if let Some(s) = start {
            if seconds < s {
                continue;
            }
        }
        if let Some(e) = end {
            if seconds > e {
                continue;
            }
        }

        let author = commit.author();
        let mut entry = String::new();
        writeln!(entry, "Commit: {}", oid).unwrap();
        writeln!(
            entry,
            "Author: {} <{}>",
            author.name().unwrap_or("Unknown"),
            author.email().unwrap_or("unknown")
        )
        .unwrap();
        writeln!(
            entry,
            "Date: {}",
            format_git_time(seconds, time.offset_minutes())
        )
        .unwrap();
        writeln!(
            entry,
            "Message: {}",
            commit.summary().unwrap_or("").trim_end()
        )
        .unwrap();
        entries.push(entry);
    }

    Ok(entries.join("\n"))
}

/// Parse either an ISO-8601 / RFC-3339 absolute form, or a relative form like
/// "2 weeks ago" / "3 days". Returns a Unix timestamp in seconds.
fn parse_timestamp(raw: &str) -> Result<i64> {
    reject_flag_arg("timestamp", raw)?;
    let s = raw.trim();
    if s.is_empty() {
        bail!("empty timestamp");
    }

    if let Ok(ts) = s.parse::<jiff::Timestamp>() {
        return Ok(ts.as_second());
    }

    if let Ok(dt) = s.parse::<jiff::civil::DateTime>() {
        let zoned = dt
            .to_zoned(jiff::tz::TimeZone::system())
            .context("failed to bind civil datetime to system zone")?;
        return Ok(zoned.timestamp().as_second());
    }

    if let Ok(date) = s.parse::<jiff::civil::Date>() {
        let zoned = date
            .to_zoned(jiff::tz::TimeZone::system())
            .context("failed to bind date to system zone")?;
        return Ok(zoned.timestamp().as_second());
    }

    let trimmed = s.trim().strip_suffix(" ago").map(str::trim).unwrap_or(s);
    if let Ok(span) = trimmed.parse::<jiff::Span>() {
        // Calendar units (weeks, months) need a zoned reference point — subtract
        // from a Zoned then unwrap the timestamp.
        let now = jiff::Zoned::now();
        let before = now
            .checked_sub(span)
            .map_err(|e| anyhow!("cannot subtract span from now: {e}"))?;
        return Ok(before.timestamp().as_second());
    }

    bail!("unparseable timestamp: {s:?} (accepted: ISO-8601, YYYY-MM-DD, or '<N> <unit> ago')")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_flag_prefix() {
        assert!(parse_timestamp("--since=evil").is_err());
    }

    #[test]
    fn parses_iso_date() {
        let t = parse_timestamp("2024-01-15").unwrap();
        assert!(t > 0);
    }

    #[test]
    fn parses_relative() {
        let t = parse_timestamp("2 weeks ago").unwrap();
        let now = jiff::Timestamp::now().as_second();
        let two_weeks = 14 * 24 * 60 * 60;
        assert!((now - t - two_weeks).abs() < 120);
    }
}
