use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub mod vad;

pub fn record_to_wav(path: &str, seconds: u64) -> Result<()> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| anyhow!("no input device found"))?;
    let config = device.default_input_config()?;
    let sample_format = config.sample_format();
    let config: cpal::StreamConfig = config.into();

    let spec = hound::WavSpec {
        channels: config.channels,
        sample_rate: config.sample_rate.0,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let writer = Arc::new(Mutex::new(hound::WavWriter::create(path, spec)?));
    let writer_clone = writer.clone();
    let err_fn = |err| eprintln!("stream error: {err}");

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                let mut w = writer_clone.lock().unwrap();
                for &sample in data { let _ = w.write_sample(sample); }
            },
            err_fn, None,
        )?,
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| {
                let mut w = writer_clone.lock().unwrap();
                for &sample in data { let _ = w.write_sample(sample as f32 / i16::MAX as f32); }
            },
            err_fn, None,
        )?,
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };

    stream.play()?;
    std::thread::sleep(Duration::from_secs(seconds));
    drop(stream);

    let writer = Arc::try_unwrap(writer).map_err(|_| anyhow!("writer still has other references"))?.into_inner().unwrap();
    writer.finalize()?;
    Ok(())
}

/// A live microphone stream. Keep `stream` alive as long as you want to
/// keep capturing — dropping it stops the mic. Raw chunks arrive on `rx`.
pub struct LiveStream {
    pub stream: cpal::Stream,
    pub rx: std::sync::mpsc::Receiver<Vec<f32>>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub fn start_stream() -> Result<LiveStream> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| anyhow!("no input device found"))?;
    let config = device.default_input_config()?;
    let sample_format = config.sample_format();
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let config: cpal::StreamConfig = config.into();

    let (tx, rx) = std::sync::mpsc::channel::<Vec<f32>>();
    let err_fn = |err| eprintln!("stream error: {err}");

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| { let _ = tx.send(data.to_vec()); },
            err_fn, None,
        )?,
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| {
                let converted: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                let _ = tx.send(converted);
            },
            err_fn, None,
        )?,
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };

    stream.play()?;
    Ok(LiveStream { stream, rx, sample_rate, channels })
}

pub fn downmix(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 { return interleaved.to_vec(); }
    let channels = channels as usize;
    interleaved.chunks(channels).map(|frame| frame.iter().sum::<f32>() / frame.len() as f32).collect()
}

pub fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() { return samples.to_vec(); }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = (samples.len() as f64 * ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = *samples.get(idx).unwrap_or(&0.0);
        let b = *samples.get(idx + 1).unwrap_or(&a);
        out.push(a + (b - a) * frac);
    }
    out
}