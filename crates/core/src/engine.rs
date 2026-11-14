use crate::{
    backend::{CleanupBackend, Cohere, Llama, SpeechBackend},
    platform::{self, Recording},
    store::{self, Entry, Settings},
    Mode,
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const IDLE: Duration = Duration::from_secs(300);
#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub phase: String,
    pub message: String,
    pub models_loaded: bool,
    pub last: Option<Entry>,
    pub level: f32,
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Start,
    Stop,
    Cancel,
    Test { text: String },
    Save { settings: Settings },
    ClearHistory,
}
struct Request {
    action: Action,
    reply: mpsc::SyncSender<std::result::Result<Value, String>>,
}
#[derive(Clone)]
pub struct Engine {
    tx: mpsc::SyncSender<Request>,
    pub status: Arc<Mutex<Status>>,
    pub dir: PathBuf,
}
impl Engine {
    pub fn start(dir: PathBuf) -> Result<Self> {
        let settings_path = dir.join("settings.json");
        let settings = if settings_path.exists() {
            store::read(&settings_path)?
        } else {
            let s = Settings::default();
            store::save(&settings_path, &s)?;
            s
        };
        let (tx, rx) = mpsc::sync_channel::<Request>(8);
        let status = Arc::new(Mutex::new(Status {
            phase: "idle".into(),
            message: "Ready when you are".into(),
            ..Status::default()
        }));
        let engine = Self {
            tx,
            status: status.clone(),
            dir: dir.clone(),
        };
        std::thread::spawn(move || {
            let mut state = State {
                dir,
                settings,
                recording: None,
                asr: None,
                cleanup: None,
                touched: Instant::now(),
                status,
            };
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(request) => {
                        let result = state.handle(request.action).map_err(|e| format!("{e:#}"));
                        if let Err(error) = &result {
                            if state.recording.is_some() {
                                state.set("recording", error);
                            } else {
                                let _ = std::fs::remove_file(state.dir.join("recording.wav"));
                                state.set("error", error);
                            }
                        }
                        let _ = request.reply.send(result);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        state.status.lock().unwrap().level =
                            state.recording.as_ref().map_or(0.0, |r| {
                                f32::from_bits(r.level.load(std::sync::atomic::Ordering::Relaxed))
                            });
                        if state
                            .recording
                            .as_ref()
                            .is_some_and(|r| r.elapsed() >= Duration::from_secs(90))
                        {
                            let _ = state.recording.take();
                            let _ = std::fs::remove_file(state.dir.join("recording.wav"));
                            state.set("error", "90 second recording limit reached. Recording discarded; try a shorter dictation.");
                        }
                        state.unload_idle();
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Ok(engine)
    }
    pub fn action(&self, action: Action) -> std::result::Result<Value, String> {
        let mut status = self.status.lock().unwrap();
        let stopping = matches!(action, Action::Stop | Action::Cancel);
        if matches!(status.phase.as_str(), "transcribing" | "cleaning")
            || (status.phase == "starting" && !stopping)
        {
            return Err("A job is already running".into());
        }
        let reserving = matches!(action, Action::Start | Action::Test { .. });
        if reserving && status.phase == "recording" {
            return Err("Already recording".into());
        }
        let (tx, rx) = mpsc::sync_channel(1);
        self.tx
            .try_send(Request { action, reply: tx })
            .map_err(|_| "Command queue is busy".to_string())?;
        if reserving {
            status.phase = "starting".into();
        } else if stopping && matches!(status.phase.as_str(), "recording" | "starting") {
            // Hide the recording overlay as soon as release is queued, before WAV finalization.
            status.phase = "transcribing".into();
        }
        drop(status);
        rx.recv_timeout(Duration::from_secs(420))
            .map_err(|_| "Job timed out".to_string())?
    }
    pub fn snapshot(&self) -> Value {
        json!({"status": self.status.lock().unwrap().clone(),
            "settings": store::read::<Settings>(&self.dir.join("settings.json")).ok(),
            "history": store::history(&self.dir).unwrap_or_default()})
    }
}
struct State {
    dir: PathBuf,
    settings: Settings,
    recording: Option<Recording>,
    asr: Option<Box<dyn SpeechBackend>>,
    cleanup: Option<Box<dyn CleanupBackend>>,
    touched: Instant,
    status: Arc<Mutex<Status>>,
}
impl State {
    fn unload_idle(&mut self) {
        if self.recording.is_none() && self.touched.elapsed() >= IDLE {
            self.asr = None;
            self.cleanup = None;
            self.status.lock().unwrap().models_loaded = false;
        }
    }
    fn set(&self, phase: &str, message: &str) {
        let mut status = self.status.lock().unwrap();
        status.phase = phase.into();
        status.message = message.into();
    }
    fn handle(&mut self, action: Action) -> Result<Value> {
        match action {
            Action::Start => {
                if self.recording.is_some() {
                    bail!("Already recording");
                }
                self.recording = Some(Recording::start(
                    &self.dir.join("recording.wav"),
                    self.settings.auto_paste,
                )?);
                self.set("recording", "Listening");
            }
            Action::Cancel => {
                self.recording = None;
                let _ = std::fs::remove_file(self.dir.join("recording.wav"));
                self.set("idle", "Recording cancelled");
            }
            Action::Stop => {
                if let Some(recording) = self.recording.take() {
                    self.set("transcribing", "Transcribing on your device");
                    let target = recording.stop()?;
                    let result = self.transcribe();
                    self.touched = Instant::now();
                    let _ = std::fs::remove_file(self.dir.join("recording.wav"));
                    let raw = result?;
                    self.finish(raw, Some(target))?;
                }
            }
            Action::Test { text } => {
                if self.recording.is_some() {
                    bail!("Finish recording first");
                }
                if text.len() > 10000 {
                    bail!("Test text is too long");
                }
                self.finish(text, None)?;
            }
            Action::Save { settings } => {
                if self.recording.is_some() {
                    bail!("Finish recording before changing settings");
                }
                if settings.gpu_layers > 36
                    || settings.vocabulary.len() > 4000
                    || settings.custom.len() > 4000
                {
                    bail!("Settings exceed supported limits");
                }
                store::save(&self.dir.join("settings.json"), &settings)?;
                self.settings = settings;
                self.asr = None;
                self.cleanup = None;
                self.status.lock().unwrap().models_loaded = false;
                self.set("idle", "Settings saved");
            }
            Action::ClearHistory => {
                store::save(&self.dir.join("history.json"), &Vec::<Entry>::new())?;
                self.status.lock().unwrap().last = None;
                self.set("idle", "History deleted");
            }
        }
        Ok(json!({"ok": true}))
    }
    fn transcribe(&mut self) -> Result<String> {
        if self.asr.is_none() {
            self.asr = Some(Box::new(Cohere::start(&self.settings, &self.dir)?));
        }
        let result = self
            .asr
            .as_mut()
            .unwrap()
            .transcribe(&self.dir.join("recording.wav"));
        if result.is_err() {
            self.asr = None;
        }
        result
    }
    fn finish(&mut self, raw: String, target: Option<String>) -> Result<()> {
        if raw.trim().is_empty() {
            bail!("No speech detected");
        }
        let mut entry = Entry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            raw: raw.clone(),
            text: raw.clone(),
            mode: self.settings.mode,
            error: None,
        };
        self.status.lock().unwrap().last = Some(entry.clone());
        if self.settings.keep_history {
            entry.error = Some("Cleanup did not complete. Raw transcript preserved.".into());
            store::append(&self.dir, entry.clone())?;
            entry.error = None;
        }
        let cleanup = if self.settings.mode == Mode::Raw {
            Ok(raw)
        } else {
            self.set("cleaning", "Finding your final words");
            self.clean(&raw)
        };
        self.touched = Instant::now();
        self.status.lock().unwrap().models_loaded = self.asr.is_some() || self.cleanup.is_some();
        match cleanup {
            Ok(text) => entry.text = text,
            Err(error) => {
                self.cleanup = None;
                self.status.lock().unwrap().models_loaded = self.asr.is_some();
                entry.error = Some(format!(
                    "Cleanup failed: {error:#}. Raw transcript preserved."
                ));
            }
        }
        // Save before touching the clipboard so paste failures cannot lose the transcript.
        self.status.lock().unwrap().last = Some(entry.clone());
        if self.settings.keep_history {
            store::append(&self.dir, entry.clone())?;
        }
        if let Some(error) = entry.error {
            bail!("{error}");
        }
        if let Some(target) = target {
            platform::insert(&entry.text, &target, self.settings.auto_paste)?;
        }
        self.set("idle", "Done. Your words are ready.");
        Ok(())
    }
    fn clean(&mut self, text: &str) -> Result<String> {
        if self.cleanup.is_none() {
            self.cleanup = Some(Box::new(Llama::start(&self.settings)?));
        }
        self.cleanup.as_mut().unwrap().cleanup(text, &self.settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FailingCleanup;
    impl CleanupBackend for FailingCleanup {
        fn cleanup(&mut self, _: &str, _: &Settings) -> Result<String> {
            bail!("CUDA out of memory")
        }
    }
    fn state() -> State {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        store::private_dir(&dir).unwrap();
        State {
            dir,
            settings: Settings::default(),
            recording: None,
            asr: None,
            cleanup: Some(Box::new(FailingCleanup)),
            touched: Instant::now(),
            status: Arc::new(Mutex::new(Status::default())),
        }
    }
    #[test]
    fn cleanup_failure_preserves_raw_and_does_not_duplicate_history() {
        let mut state = state();
        assert!(state
            .finish("Keep the timeout at 300 seconds.".into(), None)
            .is_err());
        let history = store::history(&state.dir).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].raw, "Keep the timeout at 300 seconds.");
        assert!(history[0].error.as_ref().unwrap().contains("out of memory"));
        assert!(state.cleanup.is_none());
        std::fs::remove_dir_all(state.dir).unwrap();
    }
    #[test]
    fn raw_mode_bypasses_cleanup() {
        let mut state = state();
        state.settings.mode = Mode::Raw;
        state.finish("um actually no".into(), None).unwrap();
        assert_eq!(
            store::history(&state.dir).unwrap()[0].text,
            "um actually no"
        );
        std::fs::remove_dir_all(state.dir).unwrap();
    }
    #[test]
    fn release_leaves_recording_phase_before_worker_finishes() {
        let (tx, rx) = mpsc::sync_channel::<Request>(1);
        let status = Arc::new(Mutex::new(Status {
            phase: "recording".into(),
            ..Status::default()
        }));
        let engine = Engine {
            tx,
            status: status.clone(),
            dir: PathBuf::new(),
        };
        let worker = std::thread::spawn(move || {
            let request = rx.recv().unwrap();
            assert_ne!(status.lock().unwrap().phase, "recording");
            request.reply.send(Ok(json!({"ok":true}))).unwrap();
        });
        engine.action(Action::Stop).unwrap();
        worker.join().unwrap();
    }
    #[test]
    fn models_stay_warm_until_five_minutes() {
        let mut state = state();
        state.touched = Instant::now() - Duration::from_secs(299);
        state.unload_idle();
        assert!(state.cleanup.is_some());
        state.touched = Instant::now() - IDLE;
        state.unload_idle();
        assert!(state.cleanup.is_none());
        state.touched = Instant::now();
        state.cleanup = Some(Box::new(FailingCleanup));
        state.unload_idle();
        assert!(state.cleanup.is_some());
        std::fs::remove_dir_all(state.dir).unwrap();
    }
}
