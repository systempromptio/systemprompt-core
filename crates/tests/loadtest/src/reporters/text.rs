use crate::config::Thresholds;
use crate::metrics::{MetricsSnapshot, Report};

pub fn print(report: &Report, thresholds: &Thresholds) -> bool {
    let mut all_passed = true;

    println!("\n{:=<70}", "");
    println!("  Load Test Results");
    println!("{:=<70}\n", "");

    for (name, scenario) in &report.scenarios {
        println!("  {name}:");
        print_snapshot(&scenario.aggregate, "    ");

        if !check(&scenario.aggregate, thresholds) {
            all_passed = false;
        }

        for (node, snapshot) in &scenario.per_node {
            println!("    {node}:");
            print_snapshot(snapshot, "      ");
        }

        println!();
    }

    if all_passed {
        println!("  All thresholds passed.");
    } else {
        println!("  Some thresholds FAILED.");
    }

    println!("{:=<70}\n", "");
    all_passed
}

fn print_snapshot(snapshot: &MetricsSnapshot, indent: &str) {
    println!("{indent}requests:   {}", snapshot.total());
    println!("{indent}p50:        {}ms", snapshot.p50().as_millis());
    println!("{indent}p95:        {}ms", snapshot.p95().as_millis());
    println!("{indent}p99:        {}ms", snapshot.p99().as_millis());
    println!("{indent}error rate: {:.2}%", snapshot.error_rate() * 100.0);

    let served_by = snapshot.served_by();
    if !served_by.is_empty() {
        println!("{indent}served by:");
        for (instance, count) in served_by {
            let share = if snapshot.total() == 0 {
                0.0
            } else {
                *count as f64 / snapshot.total() as f64 * 100.0
            };
            println!("{indent}  {instance:<24} {count:>8}  ({share:.1}%)");
        }
    }

    let time_series = snapshot.time_series();
    if !time_series.is_empty() {
        println!("{indent}time series:");
        println!(
            "{indent}  {:>10}  {:>8}  {:>9}  {:>7}  {:>7}  {:>7}",
            "window(s)", "requests", "err rate", "p50", "p95", "p99"
        );
        for point in time_series {
            println!(
                "{indent}  {:>10}  {:>8}  {:>8.2}%  {:>5}ms  {:>5}ms  {:>5}ms",
                point.window_start_secs,
                point.requests,
                point.error_rate * 100.0,
                point.p50_ms,
                point.p95_ms,
                point.p99_ms,
            );
        }
    }
}

fn check(snapshot: &MetricsSnapshot, thresholds: &Thresholds) -> bool {
    let mut passed = true;

    if snapshot.p95().as_millis() as u64 > thresholds.p95_ms {
        eprintln!(
            "  FAIL p95 latency: {}ms > {}ms",
            snapshot.p95().as_millis(),
            thresholds.p95_ms
        );
        passed = false;
    }

    if snapshot.p99().as_millis() as u64 > thresholds.p99_ms {
        eprintln!(
            "  FAIL p99 latency: {}ms > {}ms",
            snapshot.p99().as_millis(),
            thresholds.p99_ms
        );
        passed = false;
    }

    if snapshot.error_rate() > thresholds.max_error_rate {
        eprintln!(
            "  FAIL error rate: {:.2}% > {:.2}%",
            snapshot.error_rate() * 100.0,
            thresholds.max_error_rate * 100.0
        );
        passed = false;
    }

    passed
}
