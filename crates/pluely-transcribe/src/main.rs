//! Local Whisper sidecar: one model, multiple bounded JSONL audio requests.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Cursor, Read, Write};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    id: String,
    wav_base64: String,
    language: String,
}
#[derive(Serialize)]
struct Response {
    id: String,
    text: String,
    error: Option<String>,
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Local transcription worker: {e}");
        std::process::exit(1)
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("A model path is required")?;
    let ctx = WhisperContext::new_with_params(&path, WhisperContextParameters::default())?;
    eprintln!("{{\"event\":\"ready\"}}");
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    loop {
        let mut raw = Vec::new();
        let n = input
            .by_ref()
            .take(2 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut raw)?;
        if n == 0 {
            break;
        }
        if raw.len() > 2 * 1024 * 1024 {
            return Err("Audio request exceeds budget".into());
        }
        let request: Request = serde_json::from_slice(&raw)?;
        let result = transcribe(&ctx, &request);
        let response = match result {
            Ok(text) => Response {
                id: request.id,
                text,
                error: None,
            },
            Err(_) => Response {
                id: request.id,
                text: String::new(),
                error: Some(
                    "Local transcription failed. Verify model compatibility and available memory."
                        .into(),
                ),
            },
        };
        serde_json::to_writer(&mut output, &response)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}
fn transcribe(ctx: &WhisperContext, r: &Request) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = STANDARD.decode(&r.wav_base64)?;
    let mut reader = hound::WavReader::new(Cursor::new(bytes))?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16000 || spec.bits_per_sample != 16 {
        return Err("Expected mono 16kHz PCM16 WAV".into());
    }
    let samples = reader
        .samples::<i16>()
        .map(|x| x.map(|v| v as f32 / 32768.))
        .collect::<Result<Vec<_>, _>>()?;
    if samples.is_empty() || samples.len() > 320000 {
        return Err("Audio duration out of range".into());
    }
    let mut state = ctx.create_state()?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(if r.language == "auto" {
        None
    } else {
        Some(&r.language)
    });
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_n_threads(
        std::thread::available_parallelism()
            .map(|v| v.get().min(8))
            .unwrap_or(4) as i32,
    );
    state.full(params, &samples)?;
    let mut text = String::new();
    for segment in state.as_iter() {
        text.push_str(&segment.to_string())
    }
    Ok(text.trim().to_owned())
}
