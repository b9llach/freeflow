use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

pub struct Recorder {
    shared: Arc<Mutex<Vec<f32>>>,
    stop_flag: Arc<AtomicBool>,
    join: JoinHandle<()>,
    input_sample_rate: u32,
    input_channels: u16,
}

impl Recorder {
    pub fn start() -> anyhow::Result<Self> {
        let shared: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(16_000 * 30)));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let (ready_tx, ready_rx) = mpsc::channel::<anyhow::Result<(u32, u16)>>();
        let shared_th = shared.clone();
        let stop_th = stop_flag.clone();

        let join = thread::spawn(move || {
            let built: anyhow::Result<(cpal::Stream, u32, u16)> = (|| {
                let host = cpal::default_host();
                let device = host
                    .default_input_device()
                    .ok_or_else(|| anyhow::anyhow!("no default input device"))?;
                let config = device.default_input_config()?;
                let sr = config.sample_rate().0;
                let ch = config.channels();
                let sample_format = config.sample_format();
                let stream_config: cpal::StreamConfig = config.into();
                let err_fn = |e| tracing::error!(error = ?e, "audio stream error");

                let stream = match sample_format {
                    cpal::SampleFormat::F32 => {
                        let buf = shared_th.clone();
                        device.build_input_stream(
                            &stream_config,
                            move |data: &[f32], _| {
                                buf.lock().extend_from_slice(data);
                            },
                            err_fn,
                            None,
                        )?
                    }
                    cpal::SampleFormat::I16 => {
                        let buf = shared_th.clone();
                        device.build_input_stream(
                            &stream_config,
                            move |data: &[i16], _| {
                                let mut g = buf.lock();
                                g.extend(data.iter().map(|s| *s as f32 / i16::MAX as f32));
                            },
                            err_fn,
                            None,
                        )?
                    }
                    cpal::SampleFormat::U16 => {
                        let buf = shared_th.clone();
                        device.build_input_stream(
                            &stream_config,
                            move |data: &[u16], _| {
                                let mut g = buf.lock();
                                g.extend(data.iter().map(|s| {
                                    (*s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0)
                                }));
                            },
                            err_fn,
                            None,
                        )?
                    }
                    other => anyhow::bail!("unsupported sample format: {other:?}"),
                };
                stream.play()?;
                Ok((stream, sr, ch))
            })();

            match built {
                Ok((stream, sr, ch)) => {
                    let _ = ready_tx.send(Ok((sr, ch)));
                    while !stop_th.load(Ordering::Relaxed) {
                        thread::sleep(std::time::Duration::from_millis(20));
                    }
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        });

        let (input_sample_rate, input_channels) = ready_rx
            .recv()
            .map_err(|e| anyhow::anyhow!("recorder init: {e}"))??;

        Ok(Self {
            shared,
            stop_flag,
            join,
            input_sample_rate,
            input_channels,
        })
    }

    pub fn stop_and_take(self) -> Vec<f32> {
        self.stop_flag.store(true, Ordering::Relaxed);
        let _ = self.join.join();

        let raw = std::mem::take(&mut *self.shared.lock());

        let mono = if self.input_channels > 1 {
            downmix(&raw, self.input_channels as usize)
        } else {
            raw
        };

        if self.input_sample_rate == TARGET_SAMPLE_RATE {
            mono
        } else {
            resample_linear(&mono, self.input_sample_rate, TARGET_SAMPLE_RATE)
        }
    }
}

fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    let frames = interleaved.len() / channels;
    let mut out = Vec::with_capacity(frames);
    for i in 0..frames {
        let mut sum = 0.0f32;
        for c in 0..channels {
            sum += interleaved[i * channels + c];
        }
        out.push(sum / channels as f32);
    }
    out
}

fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == to {
        return input.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = (input.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}
