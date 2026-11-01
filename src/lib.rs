//! Parsing and aggregation for web access logs.
//!
//! Supports the NCSA **Common Log Format** and the **Combined Log Format**
//! (Common plus referer and user-agent), which is what Apache and nginx emit by
//! default:
//!
//! ```text
//! 127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /page HTTP/1.0" 200 2326
//! ```
//!
//! The parser is deliberately small and dependency-free: it pulls out the
//! fields that matter for traffic statistics and ignores the rest.

use std::collections::{BTreeMap, HashMap};

/// One parsed access-log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub ip: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub bytes: u64,
}

/// Parse a single log line. Returns `None` if the line does not look like a
/// Common/Combined log record.
///
/// The request is taken from the first quoted field and split into method and
/// path; a `"-"` byte count is treated as zero, as servers write for responses
/// with no body.
pub fn parse_line(line: &str) -> Option<LogEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // IP is the first whitespace-delimited token.
    let ip = line.split_whitespace().next()?.to_string();
    if ip.is_empty() {
        return None;
    }

    // The request line sits inside the first pair of double quotes.
    let first_quote = line.find('"')?;
    let rest = &line[first_quote + 1..];
    let end_quote = rest.find('"')?;
    let request = &rest[..end_quote];
    let after_request = rest[end_quote + 1..].trim();

    let mut req_parts = request.split_whitespace();
    let method = req_parts.next()?.to_string();
    let path = req_parts.next().unwrap_or("-").to_string();

    // Status and byte count are the first two tokens after the request.
    let mut tail = after_request.split_whitespace();
    let status: u16 = tail.next()?.parse().ok()?;
    let bytes: u64 = match tail.next() {
        Some("-") | None => 0,
        Some(tok) => tok.parse().ok()?,
    };

    Some(LogEntry {
        ip,
        method,
        path,
        status,
        bytes,
    })
}

/// Accumulated statistics over many log entries.
#[derive(Debug, Default)]
pub struct Stats {
    pub total: usize,
    pub parse_errors: usize,
    pub total_bytes: u64,
    pub by_status: BTreeMap<u16, usize>,
    pub by_ip: HashMap<String, usize>,
    pub by_path: HashMap<String, usize>,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one entry into the running totals.
    pub fn add(&mut self, entry: &LogEntry) {
        self.total += 1;
        self.total_bytes += entry.bytes;
        *self.by_status.entry(entry.status).or_insert(0) += 1;
        *self.by_ip.entry(entry.ip.clone()).or_insert(0) += 1;
        *self.by_path.entry(entry.path.clone()).or_insert(0) += 1;
    }

    /// Build statistics from an iterator of lines, optionally keeping only the
    /// entries whose status code equals `status_filter`.
    pub fn from_lines<I>(lines: I, status_filter: Option<u16>) -> Stats
    where
        I: IntoIterator<Item = String>,
    {
        let mut stats = Stats::new();
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            match parse_line(&line) {
                Some(entry) => {
                    if status_filter.is_none_or(|s| s == entry.status) {
                        stats.add(&entry);
                    }
                }
                None => stats.parse_errors += 1,
            }
        }
        stats
    }

    /// Count of entries with a 4xx or 5xx status.
    pub fn error_responses(&self) -> usize {
        self.by_status
            .iter()
            .filter(|(code, _)| **code >= 400)
            .map(|(_, n)| *n)
            .sum()
    }
}

/// Return the `n` most frequent keys of a map, most frequent first. Ties are
/// broken by key so the ordering is deterministic.
pub fn top_n(map: &HashMap<String, usize>, n: usize) -> Vec<(String, usize)> {
    let mut items: Vec<(String, usize)> = map.iter().map(|(k, v)| (k.clone(), *v)).collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.truncate(n);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMBINED: &str = concat!(
        r#"203.0.113.5 - - [10/Oct/2026:13:55:36 +0000] "GET /index.html HTTP/1.1" 200 2326 "#,
        r#""https://ref" "Mozilla/5.0""#
    );

    #[test]
    fn parses_common_log_format() {
        let e = parse_line(
            r#"127.0.0.1 - frank [10/Oct/2026:13:55:36 +0000] "GET /page HTTP/1.0" 200 2326"#,
        )
        .expect("should parse");
        assert_eq!(e.ip, "127.0.0.1");
        assert_eq!(e.method, "GET");
        assert_eq!(e.path, "/page");
        assert_eq!(e.status, 200);
        assert_eq!(e.bytes, 2326);
    }

    #[test]
    fn parses_combined_log_format() {
        let e = parse_line(COMBINED).expect("should parse");
        assert_eq!(e.ip, "203.0.113.5");
        assert_eq!(e.path, "/index.html");
        assert_eq!(e.bytes, 2326);
    }

    #[test]
    fn dash_byte_count_is_zero() {
        let e = parse_line(r#"10.0.0.1 - - [x] "GET /favicon.ico HTTP/1.1" 404 -"#).unwrap();
        assert_eq!(e.status, 404);
        assert_eq!(e.bytes, 0);
    }

    #[test]
    fn rejects_garbage_lines() {
        assert!(parse_line("").is_none());
        assert!(parse_line("not a log line at all").is_none());
        assert!(parse_line(r#"1.2.3.4 - - [x] "GET /" notanumber 10"#).is_none());
    }

    #[test]
    fn stats_aggregate_and_filter() {
        let lines = vec![
            r#"1.1.1.1 - - [x] "GET /a HTTP/1.1" 200 100"#.to_string(),
            r#"1.1.1.1 - - [x] "GET /a HTTP/1.1" 200 100"#.to_string(),
            r#"2.2.2.2 - - [x] "GET /b HTTP/1.1" 404 50"#.to_string(),
            "garbage".to_string(),
            "".to_string(),
        ];
        let stats = Stats::from_lines(lines.clone(), None);
        assert_eq!(stats.total, 3);
        assert_eq!(stats.parse_errors, 1);
        assert_eq!(stats.total_bytes, 250);
        assert_eq!(stats.by_status[&200], 2);
        assert_eq!(stats.by_status[&404], 1);
        assert_eq!(stats.error_responses(), 1);

        // Filtering to 200 keeps only the two successful requests.
        let only_ok = Stats::from_lines(lines, Some(200));
        assert_eq!(only_ok.total, 2);
        assert_eq!(only_ok.error_responses(), 0);
    }

    #[test]
    fn top_n_is_sorted_and_deterministic() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), 3);
        map.insert("b".to_string(), 5);
        map.insert("c".to_string(), 5); // tie with b, broken by key
        let top = top_n(&map, 2);
        assert_eq!(top, vec![("b".to_string(), 5), ("c".to_string(), 5)]);
    }
}
