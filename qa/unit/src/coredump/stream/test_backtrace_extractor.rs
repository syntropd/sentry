//! 1:1 Unit QA tests for backtrace extractor.

use sentry_driver::coredump::extract_backtrace;

#[test]
fn test_extract_backtrace_standard_format() {
    let message = "\
Process 16783 (test_crash) of user 1000 dumped core.

Stack trace of thread 16783:
#0  0x00007fd703ba9ccc __pthread_kill_implementation (libc.so.6 + 0x74ccc)
#1  0x00007fd703b4ee8e raise (libc.so.6 + 0x19e8e)
#2  0x000000000042fea9 n/a (/tmp/test_crash + 0x2fea9)

Some trailing journal metadata
";

    let bt = extract_backtrace(message, 10).expect("Must extract backtrace");
    assert!(bt.contains("#0  0x00007fd703ba9ccc"));
    assert!(bt.contains("#1  0x00007fd703b4ee8e"));
    assert!(bt.contains("#2  0x000000000042fea9"));
    assert!(!bt.contains("Some trailing journal metadata"));
}

#[test]
fn test_extract_backtrace_truncation() {
    let message = "\
Stack trace of thread 1:
#0 frame0
#1 frame1
#2 frame2
#3 frame3
";
    let bt = extract_backtrace(message, 2).expect("Must extract truncated backtrace");
    assert!(bt.contains("#0 frame0"));
    assert!(bt.contains("#1 frame1"));
    assert!(bt.contains("... [backtrace truncated by sentry]"));
    assert!(!bt.contains("#2 frame2"));
}

#[test]
fn test_extract_backtrace_non_crash_returns_none() {
    let message = "Regular log message without stack trace\nInfo line";
    assert!(extract_backtrace(message, 10).is_none());
}
