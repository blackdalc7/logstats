# logstats

A fast little **web access-log analyser** written in Rust. Point it at an Apache
or nginx access log and it reports how many requests came in, the status-code
breakdown, how much data was served, and the busiest client IPs and paths. No
external crates — just the standard library.

It understands the **Common Log Format** and the **Combined Log Format** (the
nginx/Apache default), so most logs work out of the box.

## Example

```console
$ logstats --top 3 tests/fixtures/access.log
Requests:       6
Parse errors:   1
Bytes sent:     6.3 KiB
Error responses: 2 (4xx/5xx)

Status codes:
  200  3
  302  1
  404  1
  500  1

Top 3 client IPs:
        3  203.0.113.5
        2  198.51.100.9
        1  192.0.2.44

Top 3 paths:
        3  /index.html
        1  /login
        1  /missing
```

## Build and run

```bash
cargo build --release

# analyse a file
./target/release/logstats /var/log/nginx/access.log

# from a pipe, show the 20 busiest paths
tail -n 100000 access.log | ./target/release/logstats --top 20

# only look at server errors
./target/release/logstats --status 500 access.log
```

## Options

| Option | Default | Meaning |
|---|---|---|
| `--top N` | `10` | Show the N busiest client IPs and paths |
| `--status CODE` | *(all)* | Only count requests with this HTTP status |
| `FILE` | stdin | Access log to read (`-` or omitted = standard input) |

## How it is built

```text
src/
    lib.rs    parse_line (Common/Combined), the Stats aggregator, top_n
    main.rs   argument parsing, file/stdin input, the printed report
tests/
    integration.rs   drives the public API over a fixture log
    fixtures/access.log
```

The parsing and aggregation live in the library (`lib.rs`) with the binary
(`main.rs`) only handling I/O and formatting, so the interesting logic is unit
tested directly. `parse_line` pulls the IP, request method and path, status and
byte count out of a line and returns `None` for anything that is not a log
record (so a stray line just increments a "parse errors" counter instead of
crashing). `Stats::from_lines` folds everything together and can filter by
status code as it goes.

## Tests

```bash
cargo test        # 6 unit tests + 2 integration tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

CI runs `fmt --check`, Clippy with warnings denied, the test suite and a release
build on every push.

## Licence

MIT. See [LICENSE](LICENSE).
