//! Runs the user's exact dictations without ASR, with a new server per case.
//! Usage: cargo run -p rafiflow-core --example cleanup_regressions -- report.json [case-id]
use rafiflow_core::{
    backend::{CleanupBackend, Llama},
    store::Settings,
};
use serde_json::{json, Value};

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Supply a report.json output path"))?;
    let cases: Vec<Value> = serde_json::from_str(include_str!("cleanup_cases.json"))?;
    let selected = std::env::args().nth(2);
    let settings = Settings::default();
    let mut results = Vec::new();
    for case in cases {
        if selected.as_deref().is_some_and(|id| case["id"] != id) {
            continue;
        }
        // Process isolation also rules out server-side slot/prompt cache leakage.
        let start = std::time::Instant::now();
        let output =
            Llama::start(&settings)?.cleanup(case["input"].as_str().unwrap(), &settings)?;
        let lower = output.to_lowercase();
        let mut failures = Vec::new();
        for token in case["required"].as_array().unwrap() {
            let token = token.as_str().unwrap();
            let found = if token.contains(".md") || token.contains(".toml") || token == "Please" {
                output.contains(token)
            } else {
                lower.contains(&token.to_lowercase())
            };
            if !found {
                failures.push(format!("Missing: {token}"));
            }
        }
        for token in case["forbidden"].as_array().unwrap() {
            let token = token.as_str().unwrap();
            if lower.contains(&token.to_lowercase()) {
                failures.push(format!("Unwanted: {token}"));
            }
        }
        if let Some(alternatives) = case["required_any"].as_array() {
            if !alternatives
                .iter()
                .any(|token| lower.contains(token.as_str().unwrap()))
            {
                failures.push("Missing explicit implementation prohibition".into());
            }
        }
        println!(
            "{}: {}\n{}\n{:?}\n",
            case["id"],
            if failures.is_empty() { "PASS" } else { "FAIL" },
            output,
            failures
        );
        results.push(json!({"case": case, "output": output, "failures": failures, "seconds_including_startup": start.elapsed().as_secs_f64()}));
        std::fs::write(&path, serde_json::to_vec_pretty(&results)?)?;
    }
    anyhow::ensure!(!results.is_empty(), "No matching regression case");
    anyhow::ensure!(results.iter().all(|r| r["failures"].as_array().unwrap().is_empty()), "Content checks failed; inspect the report. Passing token checks still requires human review.");
    Ok(())
}
