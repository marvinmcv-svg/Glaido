//! P2 exit check: the deterministic formatting eval set passes at ≥ 95%.

use std::collections::HashSet;

use vocalis_evals::{default_cases_path, load, run_all_deterministic, PASS_THRESHOLD};

#[test]
fn deterministic_formatting_evals() {
    let cases = load(&default_cases_path()).expect("cases load");
    assert!(
        cases.len() >= 40,
        "the suite needs at least 40 cases, has {}",
        cases.len()
    );
    let ids: HashSet<_> = cases.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), cases.len(), "case ids must be unique");
    let categories: HashSet<_> = cases.iter().map(|c| c.category.as_str()).collect();
    for want in [
        "style",
        "raw",
        "preset",
        "snippet",
        "correction",
        "wake",
        "fallback",
        "cleanup",
    ] {
        assert!(categories.contains(want), "no {want} cases");
    }
    let report = run_all_deterministic(&cases);
    println!("{}", report.summary("Deterministic"));
    assert!(
        report.rate() >= PASS_THRESHOLD,
        "{}",
        report.summary("Deterministic formatting evals below the 95% gate")
    );
}
