use std::time::Instant;

use test_log_analyzer::{
    parse_filename_from_commandline_args, process_file_parallely, process_file_sequentialy,
};

fn main() {
    let file_path = &parse_filename_from_commandline_args();

    let instant = Instant::now();
    match process_file_sequentialy(file_path) {
        Ok(log_analyzer) => {
            println!("{}", log_analyzer);
        }
        Err(e) => {
            eprintln!("Error processing file: {}", e);
        }
    }
    let sequential_time = instant.elapsed();
    println!(
        "Time taken for sequential processing: {:.2?}",
        sequential_time
    );

    let instant = Instant::now();
    match process_file_parallely(file_path) {
        Ok(log_analyzer) => {
            println!("{}", log_analyzer);
        }
        Err(e) => {
            eprintln!("Error processing file: {}", e);
        }
    }
    let parallel_time = instant.elapsed();
    println!("Time taken for parallel processing: {:.2?}", parallel_time);
}
