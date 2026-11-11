use crate::model::{Error, Result, Speaker};
use std::{collections::VecDeque, io::Cursor};
pub const TARGET_RATE: u32 = 16_000;
pub fn wav(samples: &[f32]) -> Result<Vec<u8>> {
    if samples.is_empty() || samples.len() > TARGET_RATE as usize * 20 {
        return Err(Error::Invalid(
            "Expected a nonempty audio segment no longer than 20 seconds.".into(),
        ));
    }
    let mut cursor = Cursor::new(Vec::with_capacity(samples.len() * 2 + 44));
    {
        let mut writer = hound::WavWriter::new(
            &mut cursor,
            hound::WavSpec {
                channels: 1,
                sample_rate: TARGET_RATE,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|_| Error::Invalid("Could not encode audio.".into()))?;
        for &x in samples {
            let x = x.clamp(-1.0, 1.0);
            let v = if x < 0.0 { x * 32768.0 } else { x * 32767.0 };
            writer
                .write_sample(v.round() as i16)
                .map_err(|_| Error::Invalid("Audio encoding failed.".into()))?
        }
        writer
            .finalize()
            .map_err(|_| Error::Invalid("WAV finalization failed.".into()))?;
    }
    Ok(cursor.into_inner())
}
pub fn read_wav(bytes: &[u8]) -> Result<Vec<f32>> {
    let mut r = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|_| Error::Invalid("Invalid WAV data.".into()))?;
    let s = r.spec();
    if s.channels != 1
        || s.sample_rate != TARGET_RATE
        || s.bits_per_sample != 16
        || s.sample_format != hound::SampleFormat::Int
    {
        return Err(Error::Invalid("Expected mono 16 kHz PCM16 WAV.".into()));
    }
    r.samples::<i16>()
        .map(|x| {
            x.map(|x| {
                if x < 0 {
                    x as f32 / 32768.0
                } else {
                    x as f32 / 32767.0
                }
            })
            .map_err(|_| Error::Invalid("Truncated audio.".into()))
        })
        .collect()
}
/// Stateful linear interpolation preserves timing across arbitrary callback sizes.
/// A production audio-quality benchmark is required before making accuracy claims.
pub struct Resampler {
    from: u32,
    position: f64,
    input: Vec<f32>,
}
impl Resampler {
    pub fn new(from: u32) -> Self {
        Self {
            from,
            position: 0.0,
            input: vec![],
        }
    }
    pub fn push(&mut self, samples: &[f32]) -> Vec<f32> {
        if self.from == TARGET_RATE {
            return samples.to_vec();
        }
        self.input.extend_from_slice(samples);
        let ratio = self.from as f64 / TARGET_RATE as f64;
        let mut output = vec![];
        while self.position + 1.0 < (self.input.len() as f64) {
            let i = self.position as usize;
            let fraction = (self.position - i as f64) as f32;
            output.push(self.input[i] * (1.0 - fraction) + self.input[i + 1] * fraction);
            self.position += ratio
        }
        let consumed = (self.position as usize).min(self.input.len().saturating_sub(1));
        self.input.drain(..consumed);
        self.position -= consumed as f64;
        output
    }
}
#[derive(Debug)]
pub struct Segment {
    pub role: Speaker,
    pub at_ms: u64,
    pub samples: Vec<f32>,
}
/// Bounded energy VAD is deliberately independent of Cluely's original Silero implementation.
/// 20ms analysis, 300ms pre-roll/tail, 200ms minimum voiced audio, hard 20s segment cap.
pub struct Segmenter {
    role: Speaker,
    resampler: Resampler,
    frames: VecDeque<f32>,
    pre: VecDeque<f32>,
    active: Vec<f32>,
    voice_frames: usize,
    silent_frames: usize,
    processed: u64,
    start: u64,
    noise: f32,
}
impl Segmenter {
    pub fn new(role: Speaker, rate: u32) -> Self {
        Self {
            role,
            resampler: Resampler::new(rate),
            frames: VecDeque::new(),
            pre: VecDeque::new(),
            active: vec![],
            voice_frames: 0,
            silent_frames: 0,
            processed: 0,
            start: 0,
            noise: 0.003,
        }
    }
    pub fn push(&mut self, samples: &[f32]) -> Vec<Segment> {
        self.frames.extend(self.resampler.push(samples));
        let mut output = vec![];
        while self.frames.len() >= 320 {
            let frame = self.frames.drain(..320).collect::<Vec<_>>();
            let rms = (frame.iter().map(|v| v * v).sum::<f32>() / 320.0).sqrt();
            let speech = rms > (self.noise * 2.8).max(0.008);
            if !speech {
                self.noise = (self.noise * 0.98 + rms * 0.02).clamp(0.0008, 0.015)
            }
            self.processed += 320;
            if self.active.is_empty() {
                self.pre.extend(frame.iter().copied());
                while self.pre.len() > 4800 {
                    self.pre.pop_front();
                }
                if speech {
                    self.start = self.processed.saturating_sub(self.pre.len() as u64);
                    self.active = self.pre.drain(..).collect();
                    self.voice_frames = 1;
                    self.silent_frames = 0
                }
            } else {
                self.active.extend(frame);
                if speech {
                    self.voice_frames += 1;
                    self.silent_frames = 0
                } else {
                    self.silent_frames += 1
                }
                if self.silent_frames >= 15 || self.active.len() >= TARGET_RATE as usize * 20 {
                    if let Some(s) = self.finish() {
                        output.push(s)
                    }
                }
            }
        }
        output
    }
    fn finish(&mut self) -> Option<Segment> {
        let samples = std::mem::take(&mut self.active);
        let voiced = self.voice_frames;
        self.voice_frames = 0;
        self.silent_frames = 0;
        if voiced < 10 || samples.is_empty() {
            None
        } else {
            Some(Segment {
                role: self.role.clone(),
                at_ms: self.start * 1000 / TARGET_RATE as u64,
                samples,
            })
        }
    }
    pub fn flush(&mut self) -> Option<Segment> {
        if !self.frames.is_empty() {
            self.active.extend(self.frames.drain(..))
        }
        self.finish()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_roundtrip_rails() {
        let b = wav(&[-1.0, 0.0, 1.0]).unwrap();
        let x = read_wav(&b).unwrap();
        assert_eq!(x, vec![-1.0, 0.0, 1.0]);
        assert_eq!(&b[..4], b"RIFF")
    }
    #[test]
    fn silence_does_not_make_transcript() {
        let mut s = Segmenter::new(Speaker::Me, 48000);
        assert!(s.push(&vec![0.0; 48000]).is_empty());
        assert!(s.flush().is_none())
    }
    #[test]
    fn segment_flush_preserves_speech() {
        let mut s = Segmenter::new(Speaker::Me, 16000);
        let speech = (0..16000)
            .map(|i| ((i as f32) * 0.2).sin() * 0.1)
            .collect::<Vec<_>>();
        s.push(&speech);
        let seg = s.flush().unwrap();
        assert_eq!(seg.role, Speaker::Me);
        assert!(!seg.samples.is_empty());
        assert!(seg.samples.len() <= 320000)
    }
    #[test]
    fn arbitrary_callback_resampling_preserves_length() {
        let mut s = Resampler::new(48000);
        let mut n = 0;
        for _ in 0..100 {
            n += s.push(&[0.1; 480]).len()
        }
        assert!((n as i32 - 16000).abs() <= 2)
    }
    #[test]
    fn oversized_audio_rejected() {
        assert!(wav(&vec![0.; 320001]).is_err())
    }
}
