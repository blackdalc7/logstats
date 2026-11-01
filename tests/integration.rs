//! Integration test: parse a real fixture log through the public API.

use std::fs;

use logstats::{top_n, Stats};

#[test]
fn fixture_log_statistics() {
    let text = fs::read_to_string("tests/fixtures/access.log").expect("read fixture");
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let stats = Stats::from_lines(lines, None);

    // Six valid records, one garbage line.
    assert_eq!(stats.total, 6);
    assert_eq!(stats.parse_errors, 1);

    // Status distribution.
    assert_eq!(stats.by_status[&200], 3);
    assert_eq!(stats.by_status[&302], 1);
    assert_eq!(stats.by_status[&404], 1);
    assert_eq!(stats.by_status[&500], 1);
    assert_eq!(stats.error_responses(), 2); // 404 + 500

    // Busiest IP is 203.0.113.5 with three requests.
    let top_ips = top_n(&stats.by_ip, 1);
    assert_eq!(top_ips[0], ("203.0.113.5".to_string(), 3));

    // Busiest path is /index.html with three hits.
    let top_paths = top_n(&stats.by_path, 1);
    assert_eq!(top_paths[0], ("/index.html".to_string(), 3));

    // Bytes: 2326 + 812 + 2326 + 0 + 0 + 1024 = 6488
    assert_eq!(stats.total_bytes, 6488);
}

#[test]
fn status_filter_counts_only_matching() {
    let text = fs::read_to_string("tests/fixtures/access.log").expect("read fixture");
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let stats = Stats::from_lines(lines, Some(200));
    assert_eq!(stats.total, 3);
    assert_eq!(stats.by_status.len(), 1);
}
