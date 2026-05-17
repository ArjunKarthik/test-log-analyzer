# Design and Reasoning

## Overview

This project is a log analyzer for structured lines in the format:

```
<timestamp>|<level>|<service>|<message>
```

It supports both sequential and parallel processing of log files and is designed to emphasize memory efficiency, safe Rust, and robust error handling.

## File reading and processing

### Sequential processing

The sequential path uses `std::io::BufReader<File>` and `read_line` to process the file line by line.

- `read_file` opens the file with a buffered reader.
- `process_file_sequentialy` loops over `read_line(&mut String)`.
- Each line is parsed in place and counted immediately.

This approach uses streaming I/O and never loads the full file into memory.

### Parallel processing

The parallel path batches input lines and processes each batch with Rayon.

- The file is read in chunks of lines, where each chunk is a `Vec<String>`.
- Chunks are collected into a buffer vector `vec` and dispatched to a Rayon pool for processing.
- A shared `Arc<LogAnalyzer>` is used so each worker can update counters concurrently.

This design attempts to balance parallel work with streaming input, while avoiding a shared line iterator lock.

## Parsing without unnecessary allocations

Parsing is performed with string slices and iterator splitting:

- `record_line` receives `&str` and does not allocate new strings for parsing.
- It uses `line.split('|')` to split the line into parts.
- The log level is matched from the second field.

This is a near-zero-copy parsing strategy because it operates on borrowed data and avoids constructing new `String` objects for each field.

## Where allocations are unavoidable

There are still a few unavoidable allocations:

- `read_line` writes into a reusable `String` buffer for each line in sequential processing.
- In the parallel path, each line is stored in a `Vec<String>` for a chunk.
- `Arc<LogAnalyzer>` is used for shared state in the parallel path.

The sequential path only allocates once for the line buffer and reuses it across lines. The parallel path allocates per chunk and per line as it batches input, which is the main memory overhead.

## Performance trade-offs made

### Sequential path trade-offs

- Pros:
  - Minimal memory usage
  - Streaming processing with buffered I/O
  - Good for I/O-bound workloads and very large files
- Cons:
  - Single-threaded, so it cannot leverage multiple cores for CPU-bound parsing

### Parallel path trade-offs

- Pros:
  - Uses Rayon workers for chunked line processing
  - Shared atomic counters avoid mutex contention on per-line updates
- Cons:
  - Requires temporary line buffering per batch
  - Still reads the file sequentially, so I/O is not fully parallelized
  - Added complexity and more allocations than the sequential path

## Very large file behavior

For very large files, the sequential path is the safest option:

- It uses buffered I/O and only retains one line at a time in memory.
- It is robust for gigabyte-scale files without requiring large RAM.

The parallel path can improve CPU utilization when line parsing is relatively heavy, but it also increases memory usage because it buffers chunks of lines.

If the file is truly enormous, the best behavior is to keep buffer sizes moderate and avoid reading the entire file into memory.

## Benchmark Results

Observed performance on ~500MB log file with 12217203 log lines

* `sequential` processing: `3.54s`
* `parallel` processing: `2.86s`

The parallel path reduced execution time by roughly 20% for the current sample dataset.

## Running

From the repository root, use:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo run -- sample_log.txt
```

If your log file has a different path:

```bash
cargo run -- /path/to/your/log.txt
```