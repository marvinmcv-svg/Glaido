//! `vocalis-evals`: run the formatting eval suite.
//!
//! Without `--live`, only the deterministic set runs (the same check as
//! `cargo test -p vocalis-evals`). With `--live`, the cleanup cases are also
//! sent to the real provider through the gateway's own Anthropic client and
//! post-passed exactly as the gateway does. Live runs need `ANTHROPIC_API_KEY`
//! (and honour `ANTHROPIC_BASE_URL` / `ANTHROPIC_CLEANUP_MODEL`); without a key
//! they are skipped with a message, not failed.

use std::path::PathBuf;
use std::process::ExitCode;

use vocalis_evals::{
    check_output, default_cases_path, finish, llm_request, load, run_all_deterministic, Expect, Report, PASS_THRESHOLD,
};
use vocalis_gateway::providers::{anthropic::Anthropic, Cleanup};

const USAGE: &str = "\
Usage: vocalis-evals [--live] [--cases PATH] [--only ID]

  --live        also run the cleanup cases against the provider (needs ANTHROPIC_API_KEY)
  --cases PATH  cases file (default packages/prompts/evals/formatting/cases.jsonl)
  --only ID     run one case
";

#[tokio::main]
async fn main() -> ExitCode {
    let mut live = false;
    let mut path = default_cases_path();
    let mut only: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--live" => live = true,
            "--cases" => path = PathBuf::from(args.next().unwrap_or_default()),
            "--only" => only = args.next(),
            _ => {
                eprint!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let mut cases = match load(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("could not load cases: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(id) = &only {
        cases.retain(|c| &c.id == id);
    }

    let det = run_all_deterministic(&cases);
    println!("{}", det.summary("Deterministic"));
    let mut ok = det.rate() >= PASS_THRESHOLD;

    if live {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        let Some(key) = env("ANTHROPIC_API_KEY") else {
            println!("Live cleanup cases skipped: set ANTHROPIC_API_KEY to run them against the provider.");
            return if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
        };
        let provider = Anthropic::new(key, env("ANTHROPIC_BASE_URL"), env("ANTHROPIC_CLEANUP_MODEL"));
        let mut report = Report::default();
        for case in cases.iter().filter(|c| c.is_live()) {
            let Expect::Llm {
                output: Some(checks), ..
            } = &case.expect
            else {
                continue;
            };
            let result = async {
                let req = llm_request(case)?;
                let raw = provider
                    .clean(&req.system, &req.user)
                    .await
                    .map_err(|e| format!("provider: {e}"))?;
                let out = finish(case, &raw)?;
                check_output(checks, &out).map_err(|e| format!("{e}; output: {out:?}"))
            }
            .await;
            report.record(&case.id, result);
        }
        println!("{}", report.summary(&format!("Live cleanup ({})", provider.name())));
        ok &= report.rate() >= PASS_THRESHOLD;
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
