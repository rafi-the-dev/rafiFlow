use crate::backend::Process;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub fn active_window() -> Result<String> {
    let output = Command::new("hyprctl")
        .args(["-j", "activewindow"])
        .output()
        .context("Hyprland is required for safe automatic pasting")?;
    if !output.status.success() {
        bail!("hyprctl activewindow failed");
    }
    let window: Value = serde_json::from_slice(&output.stdout)?;
    let address = window["address"]
        .as_str()
        .context("No focused Hyprland window")?;
    if address.is_empty() || address == "0x0" {
        bail!("No focused application");
    }
    Ok(address.to_owned())
}

pub fn copy(text: &str) -> Result<()> {
    let mut child = Command::new("wl-copy")
        .args(["--type", "text/plain;charset=utf-8"])
        .stdin(Stdio::piped())
        .spawn()
        .context("Install wl-clipboard")?;
    child
        .stdin
        .take()
        .context("Clipboard stdin missing")?
        .write_all(text.as_bytes())?;
    if !child.wait()?.success() {
        bail!("Clipboard copy failed");
    }
    Ok(())
}
pub fn insert(text: &str, target: &str, paste: bool) -> Result<()> {
    copy(text)?;
    if paste {
        if active_window()? != target {
            bail!("Focus changed. Text copied; paste it manually to avoid inserting into the wrong app.");
        }
        let window: Value = serde_json::from_slice(
            &Command::new("hyprctl")
                .args(["-j", "activewindow"])
                .output()?
                .stdout,
        )?;
        let terminal = window["tags"].as_array().is_some_and(|tags| {
            tags.iter().any(|tag| {
                tag.as_str()
                    .is_some_and(|tag| tag.trim_end_matches('*') == "terminal")
            })
        });
        let (mods, key) = if terminal {
            ("SHIFT", "Insert")
        } else {
            ("CTRL", "V")
        };
        let version = Command::new("hyprctl").arg("version").output()?;
        let version = String::from_utf8_lossy(&version.stdout);
        let minor = version
            .split_whitespace()
            .find(|part| part.starts_with("0."))
            .and_then(|part| part.split('.').nth(1))
            .and_then(|part| part.parse::<u32>().ok())
            .unwrap_or(0);
        if !target.starts_with("0x") || !target[2..].chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("Invalid target window");
        }
        if minor >= 55 {
            let chord = |state: &str| {
                format!("hl.dsp.send_key_state({{mods=\"{mods}\",key=\"{key}\",state=\"{state}\",window=\"address:{target}\"}})")
            };
            let down = Command::new("hyprctl")
                .args(["dispatch", &chord("down")])
                .output()?;
            std::thread::sleep(Duration::from_millis(50));
            // Always release even if the down call returned an error.
            let up = Command::new("hyprctl")
                .args(["dispatch", &chord("up")])
                .output()?;
            if !down.status.success() || !up.status.success() {
                bail!(
                    "Hyprland paste shortcut failed. Text is on the clipboard. {}",
                    String::from_utf8_lossy(&down.stdout)
                );
            }
        } else {
            let output = Command::new("hyprctl")
                .args([
                    "dispatch",
                    "sendshortcut",
                    &format!("{mods},{key},address:{target}"),
                ])
                .output()?;
            if !output.status.success() {
                bail!("Hyprland paste failed. Text is on the clipboard.");
            }
        }
    }
    Ok(())
}
pub struct Recording {
    child: Process,
    pub target: String,
    started: Instant,
    writer: Option<JoinHandle<Result<()>>>,
    pub level: Arc<AtomicU32>,
}
impl Recording {
    pub fn start(path: &Path, paste: bool) -> Result<Self> {
        let target = if paste {
            active_window()?
        } else {
            String::new()
        };
        let mut command = Command::new("pw-record");
        command
            .args([
                "--rate",
                "16000",
                "--channels",
                "1",
                "--format",
                "s16",
                "--raw",
                "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped());
        let mut child =
            crate::backend::spawn_owned(&mut command).context("Install PipeWire (pw-record)")?;
        let mut stream = child.stdout.take().context("Recorder stdout missing")?;
        let path = path.to_owned();
        let level = Arc::new(AtomicU32::new(0));
        let meter = level.clone();
        let writer = std::thread::spawn(move || {
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 16000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut wav = hound::WavWriter::create(path, spec)?;
            let mut buffer = [0u8; 4096];
            let mut pending = None;
            loop {
                let count = stream.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                let mut power = 0.0f32;
                let mut samples = 0;
                for byte in &buffer[..count] {
                    if let Some(low) = pending.take() {
                        let sample = i16::from_le_bytes([low, *byte]);
                        wav.write_sample(sample)?;
                        power += (sample as f32 / 32768.0).powi(2);
                        samples += 1;
                    } else {
                        pending = Some(*byte);
                    }
                }
                if samples > 0 {
                    meter.store(
                        ((power / samples as f32).sqrt() * 8.0).min(1.0).to_bits(),
                        Ordering::Relaxed,
                    );
                }
            }
            wav.finalize()?;
            Ok(())
        });
        Ok(Self {
            child: Process(child),
            target,
            started: Instant::now(),
            writer: Some(writer),
            level,
        })
    }
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
    pub fn stop(mut self) -> Result<String> {
        if let Some(status) = self.child.0.try_wait()? {
            bail!("Audio recorder exited early: {status}");
        }
        unsafe {
            libc::kill(self.child.0.id() as i32, libc::SIGINT);
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.child.0.try_wait()?.is_none() {
            if Instant::now() >= deadline {
                bail!("Audio recorder did not stop");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.writer
            .take()
            .context("Audio writer missing")?
            .join()
            .map_err(|_| anyhow::anyhow!("Audio writer panicked"))??;
        Ok(self.target.clone())
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        let _ = self.child.0.kill();
        let _ = self.child.0.wait();
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

/// Install compositor rules only for our recording window, without editing user files.
pub fn configure_overlay() -> Result<()> {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return Ok(());
    }
    let version = Command::new("hyprctl").arg("version").output()?;
    let version = String::from_utf8_lossy(&version.stdout);
    let minor = version
        .split_whitespace()
        .find(|part| part.starts_with("0."))
        .and_then(|part| part.split('.').nth(1))
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    if minor < 55 {
        return Ok(());
    }
    let rule = r#"hl.window_rule({name="rafiflow-overlay",match={class="^rafiflow$",title="^rafiFlow recording$"},float=true,no_focus=true,border_size=0,min_size={340,90},max_size={340,90},size={340,90},move={"(monitor_w-window_w)/2","(monitor_h-window_h-60)"}})"#;
    let output = Command::new("hyprctl").args(["eval", rule]).output()?;
    if !output.status.success() {
        bail!(
            "Configure recording overlay: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    Ok(())
}
