// Manual desktop checks. Paste mode requires an explicit test-window address.
use rafiflow_core::platform;
fn main() -> anyhow::Result<()> {
    if let Some(target) = std::env::args().nth(1) {
        platform::insert(
            "rafiFlow paste test: AGENTS.md, 1536, 300 seconds.",
            &target,
            true,
        )?;
        println!("Paste command completed for the supplied focused test window.");
    } else {
        let path = std::env::temp_dir().join(format!("rafiflow-{}.wav", uuid::Uuid::new_v4()));
        let recording = platform::Recording::start(&path, false)?;
        std::thread::sleep(std::time::Duration::from_millis(600));
        recording.stop()?;
        let audio = hound::WavReader::open(&path)?;
        assert_eq!(audio.spec().sample_rate, 16000);
        assert_eq!(audio.spec().channels, 1);
        assert_eq!(audio.spec().bits_per_sample, 16);
        assert!(audio.duration() > 0);
        println!(
            "Microphone capture produced {} samples at 16 kHz.",
            audio.duration()
        );
        std::fs::remove_file(path)?;
    }
    Ok(())
}
