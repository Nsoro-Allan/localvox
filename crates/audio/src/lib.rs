use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Records `seconds` of audio from the system's default input device
/// and writes it to a WAV file at `path`. A sanity check that mic
/// capture works before wiring it into the ASR pipeline.
pub fn record_to_wav(path: &str, seconds: u64) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no input device found"))?;

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
                for &sample in data {
                    let _ = w.write_sample(sample);
                }
            },
            err_fn,
            None,
        )?,
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| {
                let mut w = writer_clone.lock().unwrap();
                for &sample in data {
                    let _ = w.write_sample(sample as f32 / i16::MAX as f32);
                }
            },
            err_fn,
            None,
        )?,
        other => return Err(anyhow!("unsupported sample format: {other:?}")),
    };

    stream.play()?;
    std::thread::sleep(Duration::from_secs(seconds));
    drop(stream);

    let writer = Arc::try_unwrap(writer)
        .map_err(|_| anyhow!("writer still has other references"))?
        .into_inner()
        .unwrap();
    writer.finalize()?;
    Ok(())
}