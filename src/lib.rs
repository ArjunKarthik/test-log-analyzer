use rayon::{ThreadPoolBuilder, prelude::*};
use std::{
    fmt::Display,
    fs::File,
    io::{BufRead, BufReader, Lines},
    sync::{Arc, atomic::AtomicUsize},
};

pub struct LogAnalyzer {
    error_count: AtomicUsize,
    warning_count: AtomicUsize,
    info_count: AtomicUsize,
    invalid_format_count: AtomicUsize,
}

impl Default for LogAnalyzer {
    fn default() -> Self {
        LogAnalyzer {
            error_count: AtomicUsize::new(0),
            warning_count: AtomicUsize::new(0),
            info_count: AtomicUsize::new(0),
            invalid_format_count: AtomicUsize::new(0),
        }
    }
}

impl Display for LogAnalyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "ERROR:   {}",
            self.error_count.load(std::sync::atomic::Ordering::Relaxed)
        )?;
        writeln!(
            f,
            "WARN:    {}",
            self.warning_count
                .load(std::sync::atomic::Ordering::Relaxed)
        )?;
        writeln!(
            f,
            "INFO:    {}",
            self.info_count.load(std::sync::atomic::Ordering::Relaxed)
        )?;
        writeln!(
            f,
            "INVALID: {}",
            self.invalid_format_count
                .load(std::sync::atomic::Ordering::Relaxed)
        )
    }
}

impl LogAnalyzer {
    pub fn new() -> Self {
        LogAnalyzer::default()
    }

    pub fn increment_error(&self) {
        self.error_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn increment_warning(&self) {
        self.warning_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn increment_info(&self) {
        self.info_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn increment_invalid_format(&self) {
        self.invalid_format_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_line(&self, line: &str) {
        let mut parts = line.splitn(4, '|');
        if parts.clone().count() >= 4 {
            parts.next();
            match parts.next() {
                Some(LOG_LEVEL_ERROR) => self.increment_error(),
                Some(LOG_LEVEL_WARN) => self.increment_warning(),
                Some(LOG_LEVEL_INFO) => self.increment_info(),
                _ => self.increment_invalid_format(),
            }
        } else {
            self.increment_invalid_format();
        }
    }
}

const LOG_LEVEL_WARN: &str = "WARN";
const LOG_LEVEL_INFO: &str = "INFO";
const LOG_LEVEL_ERROR: &str = "ERROR";
const MAX_LINES_PER_CHUNK: usize = 10000;

pub fn open_file<P: AsRef<std::path::Path>>(file_path: P) -> Result<File, std::io::Error> {
    let file_path = file_path.as_ref();
    println!("\nOpening file: {}", file_path.display());
    File::open(file_path)
}

pub fn read_file(file_path: &str) -> Result<BufReader<File>, std::io::Error> {
    open_file(file_path).map(|file| {
        println!("File opened in read mode for log analysis...");
        BufReader::new(file)
    })
}

pub fn process_file_sequentialy(file_path: &str) -> Result<LogAnalyzer, std::io::Error> {
    let mut buf_reader = read_file(file_path)?;
    let log_analyzer = LogAnalyzer::new();
    let mut string_buf = String::new();

    while buf_reader.read_line(&mut string_buf)? > 0 {
        log_analyzer.record_line(&string_buf);
        string_buf.clear();
    }
    Ok(log_analyzer)
}

pub fn process_file_parallely(file_path: &str) -> Result<LogAnalyzer, std::io::Error> {
    let mut total_lines_read = 0;
    let mut lines: Vec<String> = Vec::new();
    let mut vec: Vec<Vec<String>> = Vec::new();
    let log_analyzer: Arc<LogAnalyzer> = Arc::new(LogAnalyzer::new());
    let mut lines_iterator: Lines<BufReader<File>> = read_file(file_path)?.lines();
    let max_thread_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(5);
    let thread_pool = ThreadPoolBuilder::new()
        .num_threads(max_thread_count)
        .build()
        .unwrap();

    let mut is_file_processed = false;
    while !is_file_processed {
        let element = lines_iterator.next();
        match element {
            Some(result) => {
                total_lines_read += 1;
                match result {
                    Ok(line) => lines.push(line),
                    Err(e) => eprintln!("Error reading line {} : {}", total_lines_read, e),
                }
            }
            None => is_file_processed = true,
        }

        if lines.len() % MAX_LINES_PER_CHUNK == 0 || is_file_processed {
            vec.push(lines);
            lines = Vec::new();
        }

        if vec.len() == max_thread_count || is_file_processed {
            thread_pool.install(|| {
                vec.par_iter().for_each(|lines| {
                    for line in lines {
                        log_analyzer.record_line(line);
                    }
                });
            });
            vec = Vec::new();
        }
    }
    Ok(Arc::try_unwrap(log_analyzer).unwrap_or_else(|_| LogAnalyzer::new()))
}

pub fn parse_filename_from_commandline_args() -> String {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("File path argument is missing. Usage: cargo run <file_path>");
        std::process::exit(1);
    }
    args[1].clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::Ordering;

    fn write_test_file(name: &str, contents: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "test_log_analyzer_{}_{}.log",
            name,
            std::process::id()
        ));
        fs::write(&path, contents).expect("failed to write test file");
        path
    }

    #[test]
    fn record_line_counts_log_levels() {
        let analyzer = LogAnalyzer::new();

        analyzer.record_line("2025-01-01T12:00:00Z|ERROR|auth|invalid token");
        analyzer.record_line("2025-01-01T12:00:00Z|WARN|auth|slow request");
        analyzer.record_line("2025-01-01T12:00:00Z|INFO|auth|user login");
        analyzer.record_line("malformed line");

        assert_eq!(analyzer.error_count.load(Ordering::Relaxed), 1);
        assert_eq!(analyzer.warning_count.load(Ordering::Relaxed), 1);
        assert_eq!(analyzer.info_count.load(Ordering::Relaxed), 1);
        assert_eq!(analyzer.invalid_format_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn sequential_processing_counts_levels() {
        let contents = "2025-01-01T12:00:00Z|ERROR|auth|invalid token\n2025-01-01T12:00:00Z|INFO|web|request ok\nmalformed line\n";
        let path = write_test_file("sequential", contents);

        let result =
            process_file_sequentialy(path.to_str().unwrap()).expect("sequential processing failed");

        assert_eq!(result.error_count.load(Ordering::Relaxed), 1);
        assert_eq!(result.warning_count.load(Ordering::Relaxed), 0);
        assert_eq!(result.info_count.load(Ordering::Relaxed), 1);
        assert_eq!(result.invalid_format_count.load(Ordering::Relaxed), 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn parallel_processing_counts_levels() {
        let contents = "2025-01-01T12:00:00Z|ERROR|auth|invalid token\n2025-01-01T12:00:00Z|WARN|web|slow request\nmalformed line\n";
        let path = write_test_file("parallel", contents);

        let result =
            process_file_parallely(path.to_str().unwrap()).expect("parallel processing failed");

        assert_eq!(result.error_count.load(Ordering::Relaxed), 1);
        assert_eq!(result.warning_count.load(Ordering::Relaxed), 1);
        assert_eq!(result.info_count.load(Ordering::Relaxed), 0);
        assert_eq!(result.invalid_format_count.load(Ordering::Relaxed), 1);

        let _ = fs::remove_file(path);
    }
}
