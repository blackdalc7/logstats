//! Command-line front-end for logstats.
//!
//! Usage: logstats [--top N] [--status CODE] [FILE]
//!   --top N        show the N busiest IPs and paths (default 10)
//!   --status CODE  only count requests with this HTTP status
//!   FILE           access log to read; omitted or "-" means stdin

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;

use logstats::{top_n, Stats};

struct Args {
    top: usize,
    status: Option<u16>,
    path: Option<String>,
}

fn parse_args(argv: &[String]) -> Result<Args, String> {
    let mut args = Args {
        top: 10,
        status: None,
        path: None,
    };
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "-h" | "--help" => return Err("help".to_string()),
            "--top" => {
                i += 1;
                let v = argv.get(i).ok_or("--top needs a value")?;
                args.top = v.parse().map_err(|_| format!("invalid --top value: {v}"))?;
            }
            "--status" => {
                i += 1;
                let v = argv.get(i).ok_or("--status needs a value")?;
                args.status = Some(
                    v.parse()
                        .map_err(|_| format!("invalid --status value: {v}"))?,
                );
            }
            "-" => args.path = None,
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unknown option: {other}"));
            }
            other => args.path = Some(other.to_string()),
        }
        i += 1;
    }
    Ok(args)
}

fn read_lines(path: &Option<String>) -> io::Result<Vec<String>> {
    let reader: Box<dyn BufRead> = match path {
        Some(p) => Box::new(BufReader::new(File::open(p)?)),
        None => Box::new(BufReader::new(io::stdin())),
    };
    reader.lines().collect()
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn report(stats: &Stats, top: usize) {
    println!("Requests:       {}", stats.total);
    println!("Parse errors:   {}", stats.parse_errors);
    println!("Bytes sent:     {}", human_bytes(stats.total_bytes));
    println!("Error responses: {} (4xx/5xx)", stats.error_responses());

    println!("\nStatus codes:");
    for (code, count) in &stats.by_status {
        println!("  {code:>3}  {count}");
    }

    println!("\nTop {top} client IPs:");
    for (ip, count) in top_n(&stats.by_ip, top) {
        println!("  {count:>7}  {ip}");
    }

    println!("\nTop {top} paths:");
    for (path, count) in top_n(&stats.by_path, top) {
        println!("  {count:>7}  {path}");
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(ref e) if e == "help" => {
            eprintln!("usage: logstats [--top N] [--status CODE] [FILE]");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("logstats: {e}");
            return ExitCode::from(2);
        }
    };

    let lines = match read_lines(&args.path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("logstats: {e}");
            return ExitCode::FAILURE;
        }
    };

    let stats = Stats::from_lines(lines, args.status);
    report(&stats, args.top);
    ExitCode::SUCCESS
}
