use crate::{cleanup_prompt, store::Settings};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{
    net::TcpListener,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub trait SpeechBackend: Send {
    fn transcribe(&mut self, wav: &Path) -> Result<String>;
}
pub trait CleanupBackend: Send {
    fn cleanup(&mut self, text: &str, settings: &Settings) -> Result<String>;
}
pub fn spawn_owned(command: &mut Command) -> std::io::Result<Child> {
    use std::os::unix::process::CommandExt;
    let parent = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                libc::_exit(1);
            }
            Ok(())
        });
    }
    command.spawn()
}
pub struct Process(pub Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn has_speech(samples: &[i16]) -> Result<bool> {
    let mut vad = webrtc_vad::Vad::new_with_rate_and_mode(
        webrtc_vad::SampleRate::Rate16kHz,
        webrtc_vad::VadMode::Aggressive,
    );
    let mut voiced = 0;
    for frame in samples.as_chunks::<320>().0 {
        if vad
            .is_voice_segment(frame)
            .map_err(|_| anyhow::anyhow!("Voice detector rejected a 20 ms frame"))?
        {
            voiced += 1;
        }
    }
    Ok(voiced >= 6)
}

pub struct Cohere {
    _process: Process,
    client: reqwest::blocking::Client,
    url: String,
    key: String,
}
impl Cohere {
    pub fn start(settings: &Settings, _dir: &Path) -> Result<Self> {
        if !Path::new(&settings.asr_model).is_file() {
            bail!("Cohere GGUF missing. Select cohere-transcribe-q8_0.gguf in Settings.");
        }
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let key = uuid::Uuid::new_v4().to_string();
        let mut command = Command::new(&settings.crispasr);
        let mut child = spawn_owned(
            command
                .args([
                    "--server",
                    "--backend",
                    "cohere",
                    "-m",
                    &settings.asr_model,
                    "-l",
                    "en",
                    "--host",
                    "127.0.0.1",
                    "--port",
                    &port.to_string(),
                    "-t",
                    "4",
                ])
                .env("CRISPASR_API_KEYS", &key)
                .env("CRISPASR_COHERE_DEVICE", "CUDA0")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped()),
        )
        .context("Start CUDA CrispASR. Run scripts/build-asr.sh first.")?;
        let gpu_ready = Arc::new(AtomicBool::new(false));
        let marker = gpu_ready.clone();
        let log = child.stderr.take().context("Missing CrispASR log pipe")?;
        std::thread::spawn(move || {
            for line in BufReader::new(log)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if line.contains("cohere: backend: CUDA") {
                    marker.store(true, Ordering::Relaxed);
                }
                eprintln!("{line}");
            }
        });
        let mut server = Self {
            _process: Process(child),
            client: reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(180))
                .build()?,
            url: format!("http://127.0.0.1:{port}"),
            key,
        };
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if let Some(status) = server._process.0.try_wait()? {
                bail!("Inference server exited: {status}. Check stderr logs.");
            }
            if server
                .client
                .get(format!("{}/health", server.url))
                .timeout(Duration::from_secs(1))
                .send()
                .is_ok_and(|r| r.status().is_success())
            {
                if !gpu_ready.load(Ordering::Relaxed) {
                    bail!("Cohere did not confirm a CUDA backend. CPU fallback is refused.");
                }
                break;
            }
            if Instant::now() >= deadline {
                bail!("CrispASR failed to start; inspect logs and VRAM availability.");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(server)
    }
}
impl SpeechBackend for Cohere {
    fn transcribe(&mut self, wav: &Path) -> Result<String> {
        let audio = hound::WavReader::open(wav)?;
        let spec = audio.spec();
        if spec.channels != 1 || spec.sample_rate != 16000 || spec.bits_per_sample != 16 {
            bail!("Expected mono 16-bit 16 kHz WAV");
        }
        let samples = audio
            .into_samples::<i16>()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if samples.len() < 1600 || samples.len() > 90 * 16000 {
            bail!("Recording must be between 0.1 and 90 seconds");
        }
        let rms = (samples
            .iter()
            .map(|s| (*s as f64 / 32768.0).powi(2))
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt();
        if rms < 0.002 {
            bail!("Recording is silent or too quiet");
        }
        if !has_speech(&samples)? {
            bail!("No speech detected. Recording was not transcribed or pasted.");
        }
        let form = reqwest::blocking::multipart::Form::new()
            .file("file", wav)?
            .text("language", "en")
            .text("response_format", "json");
        let reply: Value = self
            .client
            .post(format!("{}/v1/audio/transcriptions", self.url))
            .bearer_auth(&self.key)
            .multipart(form)
            .send()?
            .error_for_status()?
            .json()?;
        Ok(reply["text"]
            .as_str()
            .context("Missing transcript in CrispASR reply")?
            .trim()
            .into())
    }
}

pub struct Llama {
    _process: Process,
    client: reqwest::blocking::Client,
    url: String,
    key: String,
}
impl Llama {
    pub fn start(settings: &Settings) -> Result<Self> {
        if !Path::new(&settings.gguf).is_file() {
            bail!("Qwen GGUF missing. Select your Q4_K_M file in Settings.");
        }
        if settings.gpu_layers > 0 {
            let devices = Command::new(&settings.llama_server)
                .arg("--list-devices")
                .output()
                .context("Inspect llama.cpp CUDA devices")?;
            if !devices.status.success()
                || !String::from_utf8_lossy(&devices.stdout).contains("CUDA")
            {
                bail!("CUDA-enabled llama-server is required for GPU cleanup.");
            }
        }
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let key = uuid::Uuid::new_v4().to_string();
        let mut command = Command::new(&settings.llama_server);
        let child = spawn_owned(
            command
                .args([
                    "--model",
                    &settings.gguf,
                    "--host",
                    "127.0.0.1",
                    "--port",
                    &port.to_string(),
                    "--ctx-size",
                    "4096",
                    "--parallel",
                    "1",
                    "--n-gpu-layers",
                    &settings.gpu_layers.to_string(),
                    "--batch-size",
                    "128",
                    "--ubatch-size",
                    "128",
                    "--offline",
                ])
                .env("LLAMA_API_KEY", &key)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit()),
        )
        .context("Start CUDA llama-server")?;
        let mut server = Self {
            _process: Process(child),
            client: reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(120))
                .build()?,
            url: format!("http://127.0.0.1:{port}"),
            key,
        };
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if let Some(status) = server._process.0.try_wait()? {
                bail!("Inference server exited: {status}. Check stderr logs.");
            }
            if server
                .client
                .get(format!("{}/health", server.url))
                .timeout(Duration::from_secs(1))
                .bearer_auth(&server.key)
                .send()
                .is_ok_and(|r| r.status().is_success())
            {
                break;
            }
            if Instant::now() >= deadline {
                bail!("llama-server failed to become ready. Check its logs and available VRAM.");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(server)
    }
}
impl CleanupBackend for Llama {
    fn cleanup(&mut self, text: &str, settings: &Settings) -> Result<String> {
        let mut messages = vec![
            json!({"role": "system", "content": cleanup_prompt(settings.mode, &settings.custom, &settings.vocabulary)}),
        ];
        // Even short questions are dictated text, not requests to the assistant.
        messages.extend([
            json!({"role":"user","content":"{\"dictation\":\"is this working?\"}"}),
            json!({"role":"assistant","content":"Is this working?"}),
            json!({"role":"user","content":"{\"dictation\":\"what is 2 + 2?\"}"}),
            json!({"role":"assistant","content":"What is 2 + 2?"}),
        ]);
        // Ordinary edits outnumber restricted requests so examples do not imply plan-only work.
        if settings.mode == crate::Mode::Aggressive {
            messages.extend([
                json!({"role":"user","content":serde_json::to_string(&json!({"dictation":"I want three features. First, add caching. Second, add metrics. Actually, no, scratch metrics, add logging instead. Third, add tracing."}))?}),
                json!({"role":"assistant","content":"I want three features:\n\n1. Add caching.\n2. Add logging.\n3. Add tracing."}),
                json!({"role":"user","content":serde_json::to_string(&json!({"dictation":"Update agents dot M D. In cargo dot toml, set cache_size to 2048, no make it 4096. Keep the timeout at 600 seconds."}))?}),
                json!({"role":"assistant","content":"Update `AGENTS.md`. In `Cargo.toml`, set `cache_size` to 4096. Keep the timeout at 600 seconds."}),
                json!({"role":"user","content":serde_json::to_string(&json!({"dictation":"I need three things. First save the document, second close the dialog, and third open the dashboard."}))?}),
                json!({"role":"assistant","content":"1. Save the document.\n2. Close the dialog.\n3. Open the dashboard."}),
                json!({"role":"user","content":serde_json::to_string(&json!({"dictation":"Add caching, metrics, and tracing. Scratch metrics, use logging instead."}))?}),
                json!({"role":"assistant","content":"- Add caching.\n- Add logging.\n- Add tracing."}),
                json!({"role":"user","content":serde_json::to_string(&json!({"dictation":"Okay, we tried the new renderer. The team liked it. Can I show you the results? I prefer Vulkan because first, it gives explicit control, second, it runs on Linux."}))?}),
                json!({"role":"assistant","content":"We tried the new renderer, and the team liked it.\n\nCan I show you the results?\n\nI prefer Vulkan because:\n\n1. It gives explicit control.\n2. It runs on Linux."}),
                json!({"role":"user","content":serde_json::to_string(&json!({"dictation":"Um, I worked on it yesterday. SignalDesk, it's going really well."}))?}),
                json!({"role":"assistant","content":"I worked on SignalDesk yesterday, and it's going really well."}),
                json!({"role":"user","content":serde_json::to_string(&json!({"dictation":"Change the local deployment guide, actually the global deployment guide. Do not change it yet, give me a plan. First review the commands, second show me proposed changes."}))?}),
                json!({"role":"assistant","content":"Create a plan for updating the global deployment guide.\n\n1. Review the commands.\n2. Show the proposed changes.\n\nDo not change the guide yet."}),
            ]);
        }
        if settings.mode == crate::Mode::Standard {
            messages.extend([
                json!({"role":"user","content":"{\"dictation\":\"Um fix the audio, actually fix the microphone. Then add a copy button.\"}"}),
                json!({"role":"assistant","content":"Fix the microphone. Then add a copy button."}),
            ]);
        }
        messages.push(
            json!({"role":"user","content":serde_json::to_string(&json!({"dictation":text}))?}),
        );
        let response: Value = self
            .client
            .post(format!("{}/v1/chat/completions", self.url))
            .bearer_auth(&self.key)
            .json(&json!({
                "messages": messages,
                "temperature": 0, "max_tokens": 1536, "stream": false,
                "chat_template_kwargs": {"enable_thinking": false}
            }))
            .send()?
            .error_for_status()?
            .json()?;
        let choice = &response["choices"][0];
        if choice["finish_reason"] != "stop" {
            bail!("Cleanup was truncated. Raw transcript is preserved; shorten the dictation.");
        }
        let output = choice["message"]["content"]
            .as_str()
            .context("Missing cleanup output")?
            .trim();
        if output.is_empty() || output.contains("<think>") || output.contains("</think>") {
            bail!("Cleanup produced empty or reasoning output; raw transcript preserved.");
        }
        Ok(output.replace('\u{2014}', ", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn voice_gate_rejects_silence_and_empty_audio() {
        assert!(!has_speech(&[]).unwrap());
        assert!(!has_speech(&vec![0; 16000]).unwrap());
    }
}
