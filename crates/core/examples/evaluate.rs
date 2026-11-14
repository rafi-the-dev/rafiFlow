use rafiflow_core::{
    backend::{CleanupBackend, Cohere, Llama, SpeechBackend},
    store::Settings,
    Mode,
};
fn main() -> anyhow::Result<()> {
    let mut settings = Settings::default();
    let mut asr = None;
    let mut transcript = None;
    if let Some(path) = std::env::args().nth(1) {
        let mut backend = Cohere::start(&settings, &std::env::temp_dir())?;
        let start = std::time::Instant::now();
        let raw = backend.transcribe(std::path::Path::new(&path))?;
        println!("TRANSCRIPT: {raw}\nASR elapsed: {:?}", start.elapsed());
        if std::env::args().nth(2).as_deref() == Some("--asr-only") {
            return Ok(());
        }
        transcript = Some(raw);
        asr = Some(backend);
    }
    let mut cleanup = Llama::start(&settings)?;
    if let Some(raw) = transcript {
        let output = cleanup.cleanup(&raw, &settings)?;
        println!("PIPELINE CLEANUP: {output}");
        if raw.to_lowercase().contains("global agents file") {
            anyhow::ensure!(
                output.to_lowercase().contains("global"),
                "Cleanup lost the global scope"
            );
            anyhow::ensure!(
                output.to_lowercase().contains("agents") || output.contains("AGENTS"),
                "Cleanup lost the target file"
            );
        }
    }
    evaluate(&mut settings, &mut cleanup)?;
    let memory = std::process::Command::new("nvidia-smi")
        .args([
            "--query-compute-apps=process_name,used_memory",
            "--format=csv,noheader",
        ])
        .output()?;
    for line in String::from_utf8_lossy(&memory.stdout)
        .lines()
        .filter(|line| line.contains("/crispasr,") || line.starts_with("llama-server,"))
    {
        println!("GPU process memory: {line}");
    }
    drop(asr);
    Ok(())
}
fn evaluate(settings: &mut Settings, cleanup: &mut dyn CleanupBackend) -> anyhow::Result<()> {
    let cases: [(&str, &[&str], &[&str]); 4] = [
        ("Okay so I want to modify the project agents file, actually no the global one. First fix the review rules, second update GitHub handling, and third show me the diff. Wait don't modify anything yet, just give me a plan.",
            &["global", "1.", "2.", "3.", "review", "github", "diff", "do not modify"], &["project agents", "role-based", "escalation", "I'll", "Understood"]),
        ("Update src/audio.rs, no I meant src/engine.rs. Keep max_tokens at 1536 and the timeout at 300 seconds. Do not change the API.",
            &["src/engine.rs", "max_tokens", "1536", "300", "do not change"], &["src/audio.rs", "30-0", "pub struct"]),
        ("I need three things. First add a search field, second preserve the raw transcript, and third add a copy button.",
            &["1.", "2.", "3.", "search", "raw transcript", "copy"], &["I'll", "Understood"]),
        ("Set retry_count to 7, no make it 9. Keep config.toml unchanged. First inspect the logs, second show me the error. Do not restart anything.",
            &["retry_count", "9", "config.toml", "unchanged", "do not restart"], &["to 7", "restart the", "I will"]),
    ];
    for (text, required, forbidden) in cases {
        let start = std::time::Instant::now();
        let output = cleanup.cleanup(text, settings)?;
        println!(
            "RAW: {text}\nCLEAN: {output}\nElapsed: {:?}\n",
            start.elapsed()
        );
        let lower = output.to_lowercase();
        for token in required {
            anyhow::ensure!(lower.contains(token), "Missing required content: {token}");
        }
        for token in forbidden {
            anyhow::ensure!(
                !lower.contains(&token.to_lowercase()),
                "Unwanted content: {token}"
            );
        }
    }
    for mode in [Mode::Light, Mode::Standard, Mode::Custom] {
        settings.mode = mode;
        settings.custom = "Use a concise numbered list when listing tasks.".into();
        let output = cleanup.cleanup(
            "Um first fix the audio, actually fix the microphone, second add a copy button.",
            settings,
        )?;
        anyhow::ensure!(output.contains("microphone") && output.contains("copy"));
        if mode == Mode::Standard {
            anyhow::ensure!(!output.contains("actually"));
        }
        println!("MODE {mode:?}: {output}");
    }
    for mode in [Mode::Light, Mode::Standard, Mode::Aggressive, Mode::Custom] {
        settings.mode = mode;
        settings.custom = "Keep the text concise.".into();
        for question in [
            "is this working?",
            "what is 2 + 2?",
            "can you explain this bug?",
        ] {
            let output = cleanup.cleanup(question, settings)?;
            println!("QUESTION {mode:?}: {question} -> {output}");
            anyhow::ensure!(
                output.eq_ignore_ascii_case(question),
                "Cleanup answered or changed the question: {output}"
            );
        }
    }
    println!("All cleanup content checks passed. These checks supplement human reading; they cannot prove semantic fidelity.");
    Ok(())
}
