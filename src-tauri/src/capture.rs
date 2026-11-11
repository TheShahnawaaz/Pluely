use super::service::Service;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use pluely_core::{
    audio::{wav, Segmenter},
    model::{Error, Result, Speaker, Transcript},
};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
pub struct Frame {
    pub role: Speaker,
    pub rate: u32,
    pub samples: Vec<f32>,
}
pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    children: Vec<Arc<Mutex<std::process::Child>>>,
    pub started: Instant,
    pub offset: u64,
}
impl CaptureHandle {
    pub fn elapsed(&self) -> u64 {
        self.offset + self.started.elapsed().as_millis() as u64
    }
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for child in &self.children {
            let _ = child.lock().unwrap().kill();
        }
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}
impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for child in &self.children {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        }
    }
}
fn feed(
    tx: &mpsc::Sender<Frame>,
    app: &AppHandle,
    role: Speaker,
    rate: u32,
    samples: Vec<f32>,
    dropped: &AtomicBool,
) {
    if tx
        .try_send(Frame {
            role,
            rate,
            samples,
        })
        .is_err()
        && !dropped.swap(true, Ordering::Relaxed)
    {
        let _=app.emit("pluely:notice","Audio processing fell behind. Some input frames were dropped; this transcript may have gaps.");
    }
}
pub fn start(app: &AppHandle, offset: u64) -> Result<(CaptureHandle, mpsc::Receiver<Frame>)> {
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel(128);
    let mut handle = CaptureHandle {
        stop: stop.clone(),
        threads: vec![],
        children: vec![],
        started: Instant::now(),
        offset,
    };
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let mic_stop = stop.clone();
    let mic_app = app.clone();
    let mic_tx = tx.clone();
    handle.threads.push(std::thread::spawn(move||{let result=(||->Result<()>{let host=cpal::default_host();let device=host.default_input_device().ok_or_else(||Error::Invalid("No microphone is available. Check OS input settings.".into()))?;let config=device.default_input_config().map_err(|_|Error::Invalid("Cannot open the microphone. Grant microphone permission and retry.".into()))?;let rate=config.sample_rate().0;let channels=config.channels() as usize;let stream_config:cpal::StreamConfig=config.clone().into();let dropped=Arc::new(AtomicBool::new(false));
  let error_app=mic_app.clone();let error_cb=move|_error|{let _=error_app.emit("pluely:notice","Microphone capture stopped unexpectedly. Pause and resume, or check the selected input device.");};
  let stream=match config.sample_format(){
   cpal::SampleFormat::F32=>device.build_input_stream(&stream_config,move|data:&[f32],_|{let samples=data.chunks(channels).map(|v|v.iter().sum::<f32>()/channels as f32).collect();feed(&mic_tx,&mic_app,Speaker::Me,rate,samples,&dropped)},error_cb,None),
   cpal::SampleFormat::I16=>device.build_input_stream(&stream_config,move|data:&[i16],_|{let samples=data.chunks(channels).map(|v|v.iter().map(|s|*s as f32/32768.).sum::<f32>()/channels as f32).collect();feed(&mic_tx,&mic_app,Speaker::Me,rate,samples,&dropped)},error_cb,None),
   cpal::SampleFormat::U16=>device.build_input_stream(&stream_config,move|data:&[u16],_|{let samples=data.chunks(channels).map(|v|v.iter().map(|s|(*s as f32-32768.)/32768.).sum::<f32>()/channels as f32).collect();feed(&mic_tx,&mic_app,Speaker::Me,rate,samples,&dropped)},error_cb,None),
   _=>return Err(Error::Invalid("The default microphone format is unsupported. Select a PCM microphone in OS settings.".into()))
  }.map_err(|_|Error::Invalid("Microphone initialization failed. Check permission and input-device availability.".into()))?;
  stream.play().map_err(|_|Error::Invalid("The microphone could not start.".into()))?;let _=ready_tx.send(Ok(()));while !mic_stop.load(Ordering::Relaxed){std::thread::sleep(Duration::from_millis(30));}drop(stream);Ok(())})();if let Err(e)=result{let _=ready_tx.send(Err(e.to_string()));}}));
    match ready_rx.recv_timeout(Duration::from_secs(30)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            handle.stop();
            return Err(Error::Invalid(e));
        }
        Err(_) => {
            handle.stop();
            return Err(Error::Invalid(
                "Microphone startup timed out. Check the OS permission prompt.".into(),
            ));
        }
    }
    #[cfg(target_os = "macos")]
    {
        let system = mac_system(tx.clone(), app.clone(), stop.clone());
        match system {
            Ok((thread, child)) => {
                handle.threads.push(thread);
                handle.children.push(child)
            }
            Err(e) => {
                handle.stop();
                return Err(e);
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let system = windows_system(tx.clone(), app.clone(), stop.clone());
        match system {
            Ok(thread) => handle.threads.push(thread),
            Err(e) => {
                handle.stop();
                return Err(e);
            }
        }
    }
    drop(tx);
    Ok((handle, rx))
}
#[cfg(target_os = "macos")]
fn mac_system(
    tx: mpsc::Sender<Frame>,
    app: AppHandle,
    stop: Arc<AtomicBool>,
) -> Result<(JoinHandle<()>, Arc<Mutex<std::process::Child>>)> {
    use std::io::{BufRead, Read};
    let mut child = std::process::Command::new(super::models::sidecar("pluely-capture")?)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Invalid("System capture output unavailable.".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::Invalid("System capture status unavailable.".into()))?;
    let child = Arc::new(Mutex::new(child));
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let status_app = app.clone();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stderr)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                match event.get("event").and_then(|v| v.as_str()) {
                    Some("ready") => {
                        let _ = ready_tx.try_send(Ok(()));
                    }
                    Some("error") => {
                        let message = event
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("System audio capture failed.")
                            .to_owned();
                        let _ = status_app.emit("pluely:notice", &message);
                        let _ = ready_tx.try_send(Err(message));
                    }
                    _ => {}
                }
            }
        }
    });
    let thread = std::thread::spawn(move || {
        let dropped = AtomicBool::new(false);
        let mut bytes = [0u8; 9600];
        let mut remainder = None;
        while !stop.load(Ordering::Relaxed) {
            match stdout.read(&mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut data = Vec::with_capacity(n + 1);
                    if let Some(byte) = remainder.take() {
                        data.push(byte)
                    }
                    data.extend_from_slice(&bytes[..n]);
                    if data.len() % 2 == 1 {
                        remainder = data.pop()
                    }
                    let samples = data
                        .chunks_exact(2)
                        .map(|v| i16::from_le_bytes([v[0], v[1]]) as f32 / 32768.)
                        .collect();
                    feed(&tx, &app, Speaker::Them, 48000, samples, &dropped)
                }
            }
        }
    });
    match ready_rx.recv_timeout(Duration::from_secs(45)) {
        Ok(Ok(())) => Ok((thread, child)),
        result => {
            let _ = child.lock().unwrap().kill();
            let _ = thread.join();
            Err(Error::Invalid(match result{Ok(Err(e))=>e,_=>"System audio startup timed out. Grant Screen & System Audio Recording permission, then restart Pluely.".into()}))
        }
    }
}
#[cfg(target_os = "windows")]
fn windows_system(
    tx: mpsc::Sender<Frame>,
    app: AppHandle,
    stop: Arc<AtomicBool>,
) -> Result<JoinHandle<()>> {
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || {
        let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
            use wasapi::*;
            initialize_mta().ok()?;
            let enumerator = DeviceEnumerator::new()?;
            let device = enumerator.get_default_device(Direction::Render)?;
            let mut client = device.get_iaudioclient()?;
            let format = WaveFormat::new(32, 32, SampleType::Float, 48000, 2, None);
            let (_, period) = client.get_device_period()?;
            client.initialize_client(
                &format,
                Direction::Capture,
                StreamMode::PollingShared {
                    autoconvert: true,
                    buffer_duration_hns: period,
                },
                None,
            )?;
            let capture = client.get_audiocaptureclient()?;
            client.start_stream()?;
            let _ = ready_tx.send(Ok(()));
            let mut bytes = std::collections::VecDeque::new();
            let dropped = AtomicBool::new(false);
            while !stop.load(Ordering::Relaxed) {
                capture.read_from_device_to_deque(&mut bytes)?;
                while bytes.len() >= 3840 {
                    let part = bytes.drain(..3840).collect::<Vec<_>>();
                    let samples = part
                        .chunks_exact(8)
                        .map(|v| {
                            let left = f32::from_le_bytes(v[..4].try_into().unwrap());
                            let right = f32::from_le_bytes(v[4..].try_into().unwrap());
                            (left + right) * 0.5
                        })
                        .collect();
                    feed(&tx, &app, Speaker::Them, 48000, samples, &dropped)
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            client.stop_stream()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = ready_tx.try_send(Err(
                "Cannot capture system audio. Check the default Windows playback device."
                    .to_owned(),
            ));
            let _ = app.emit(
                "pluely:notice",
                "System audio capture failed. Check the default Windows playback device.",
            );
        }
    });
    match ready_rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(())) => Ok(thread),
        Ok(Err(e)) => Err(Error::Invalid(e)),
        Err(_) => Err(Error::Invalid(
            "Windows system-audio startup timed out.".into(),
        )),
    }
}
pub async fn pipeline(
    service: Arc<Service>,
    app: AppHandle,
    session_id: String,
    offset: u64,
    mut rx: mpsc::Receiver<Frame>,
    cancel: tokio_util::sync::CancellationToken,
) {
    let config = service.snapshot().settings.stt;
    let mut me: Option<Segmenter> = None;
    let mut them: Option<Segmenter> = None;
    let mut jobs = tokio::task::JoinSet::new();
    let sem = Arc::new(tokio::sync::Semaphore::new(4));
    while let Some(frame) = rx.recv().await {
        if cancel.is_cancelled() {
            break;
        }
        let role = frame.role.clone();
        let target = if role == Speaker::Me {
            &mut me
        } else {
            &mut them
        };
        let segmenter = target.get_or_insert_with(|| Segmenter::new(role, frame.rate));
        for segment in segmenter.push(&frame.samples) {
            if jobs.len() >= 20 {
                service.notify(&app,"Transcription could not keep up. A speech segment was dropped; try a faster provider or smaller local model.");
                continue;
            }
            spawn_segment(
                &mut jobs,
                service.clone(),
                app.clone(),
                session_id.clone(),
                offset,
                config.clone(),
                segment,
                sem.clone(),
                cancel.clone(),
            );
        }
        while jobs.try_join_next().is_some() {}
    }
    for segmenter in [&mut me, &mut them].into_iter().flatten() {
        if let Some(segment) = segmenter.flush() {
            spawn_segment(
                &mut jobs,
                service.clone(),
                app.clone(),
                session_id.clone(),
                offset,
                config.clone(),
                segment,
                sem.clone(),
                cancel.clone(),
            );
        }
    }
    while jobs.join_next().await.is_some() {}
}
fn spawn_segment(
    jobs: &mut tokio::task::JoinSet<()>,
    service: Arc<Service>,
    app: AppHandle,
    session_id: String,
    offset: u64,
    config: pluely_core::model::SttConfig,
    segment: pluely_core::audio::Segment,
    sem: Arc<tokio::sync::Semaphore>,
    cancel: tokio_util::sync::CancellationToken,
) {
    jobs.spawn(async move{
 let permit=tokio::select!{_=cancel.cancelled()=>return,p=sem.acquire_owned()=>match p{Ok(p)=>p,Err(_)=>return}};let _permit=permit;service.activity();let id=uuid::Uuid::new_v4().to_string();let row=Transcript{id:id.clone(),role:segment.role,text:String::new(),at_ms:offset+segment.at_ms,r#final:false};
 if service.mutate(&app,false,|s|{let target=s.sessions.iter_mut().find(|x|x.id==session_id).ok_or(Error::Cancelled)?;target.upsert_transcript(row.clone());Ok(())}).is_err(){return}
 let result=match wav(&segment.samples){Ok(wav)=>super::models::transcribe(&service,&config,wav,&cancel).await,Err(e)=>Err(e)};
 let updated=service.mutate(&app,true,|s|{let Some(target)=s.sessions.iter_mut().find(|x|x.id==session_id)else{return Ok(())};match &result{Ok(text)if !text.trim().is_empty()=>target.upsert_transcript(Transcript{text:text.clone(),r#final:true,..row}),_=>{target.transcript.retain(|x|x.id!=id);}}Ok(())});
 if let Err(e)=result{if !matches!(e,Error::Cancelled){service.notify(&app,e.to_string())}}if let Err(e)=updated{service.notify(&app,e.to_string())}
 });
}
