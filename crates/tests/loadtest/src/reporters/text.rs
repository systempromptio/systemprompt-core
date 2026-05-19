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
