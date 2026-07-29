use std::time::Duration;

use crate::harness::ClientResult;

pub fn print_report(results: Vec<ClientResult>, wall_duration: Duration) {
    let mut connect_samples = Vec::new();
    let mut request_samples = Vec::new();
    let mut dial_errors = Vec::new();
    let mut request_errors = Vec::new();

    for result in results {
        for sample in result.samples {
            connect_samples.push(sample.connect);
            request_samples.push(sample.request);
        }
        dial_errors.extend(result.dial_errors);
        request_errors.extend(result.request_errors);
    }

    let completed = connect_samples.len();
    let req_per_sec = completed as f64 / wall_duration.as_secs_f64();

    println!(
        "completed requests: {completed}   wall time: {:.2}s   req/s: {req_per_sec:.1}",
        wall_duration.as_secs_f64()
    );
    println!();
    println!(
        "{:8} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "phase", "n", "min", "p50", "p90", "p99", "p99.9", "max"
    );
    print_percentiles("connect", &mut connect_samples);
    print_percentiles("request", &mut request_samples);
    println!();
    println!("dial errors: {}", dial_errors.len());
    print_sample_errors(&dial_errors);
    println!("request errors: {}", request_errors.len());
    print_sample_errors(&request_errors);
}

fn print_percentiles(label: &str, samples: &mut [Duration]) {
    if samples.is_empty() {
        println!("{label:8} {:>8}", 0);
        return;
    }
    samples.sort_unstable();
    let pct = |p: f64| samples[(((samples.len() - 1) as f64) * p).round() as usize];
    println!(
        "{label:8} {:>8} {:>10?} {:>10?} {:>10?} {:>10?} {:>10?} {:>10?}",
        samples.len(),
        samples[0],
        pct(0.50),
        pct(0.90),
        pct(0.99),
        pct(0.999),
        samples[samples.len() - 1],
    );
}

/// Prints up to a few distinct sample error messages — enough to see what's actually failing
/// without dumping potentially thousands of near-identical lines.
fn print_sample_errors(errors: &[String]) {
    let mut seen = std::collections::HashSet::new();
    for error in errors {
        if seen.insert(error.as_str()) {
            println!("  e.g. {error}");
            if seen.len() >= 5 {
                break;
            }
        }
    }
}
