// SPDX-License-Identifier: MPL-2.0
use anyhow::{Context, Result};
use cpal::{
	SampleFormat, StreamConfig,
	traits::{DeviceTrait, HostTrait, StreamTrait},
};
use meowlouder_opus::{OpusApplication, OpusDecoder, OpusEncoder};
use std::{
	sync::{
		Arc,
		atomic::{AtomicUsize, Ordering},
		mpsc,
	},
	time::{Duration, Instant},
};

const SAMPLE_RATE: u32 = 48_000;
const FRAME_SAMPLES: usize = 960; // 20 ms at 48 kHz, mono
const RECORD_SECS: u64 = 5;

fn main() -> Result<()> {
	let host = cpal::default_host();

	// ── Input setup ───────────────────────────────────────────────────────────
	let in_device = host
		.default_input_device()
		.context("no default input device found")?;
	println!(
		"Input device: {}",
		in_device
			.description()
			.map(|d| d.name().to_string())
			.unwrap_or_else(|_| "unknown".into())
	);

	let in_supported = in_device
		.supported_input_configs()
		.context("failed to query input configs")?
		.filter(|c| {
			c.sample_format() == SampleFormat::F32
				&& c.min_sample_rate() <= SAMPLE_RATE
				&& c.max_sample_rate() >= SAMPLE_RATE
		})
		.min_by_key(|c| c.channels())
		.context("device does not support f32 samples at 48 kHz")?
		.with_sample_rate(SAMPLE_RATE);

	let in_channels = in_supported.channels() as usize;
	let in_config: StreamConfig = in_supported.into();
	println!("Stream: {SAMPLE_RATE} Hz, {in_channels} channel(s), f32");

	// ── Record ────────────────────────────────────────────────────────────────
	let (tx, rx) = mpsc::sync_channel::<Vec<f32>>(64);
	let in_stream = in_device.build_input_stream(
		&in_config,
		move |data: &[f32], _| {
			let _ = tx.try_send(data.to_vec());
		},
		|err| eprintln!("audio stream error: {err}"),
		None,
	)?;
	in_stream.play().context("failed to start audio stream")?;

	let mut encoder = OpusEncoder::new(SAMPLE_RATE as i32, 1, OpusApplication::Voip)
		.context("failed to create Opus encoder")?;

	println!("Recording for {RECORD_SECS}s...");

	let mut buffer = Vec::<f32>::new();
	let mut packets = Vec::<Vec<u8>>::new();
	let mut total_bytes: usize = 0;
	let deadline = Instant::now() + Duration::from_secs(RECORD_SECS);

	while Instant::now() < deadline {
		match rx.recv_timeout(Duration::from_millis(50)) {
			Ok(chunk) => {
				// Mix interleaved multi-channel audio down to mono.
				if in_channels == 1 {
					buffer.extend_from_slice(&chunk);
				} else {
					for frame in chunk.chunks_exact(in_channels) {
						buffer.push(frame.iter().sum::<f32>() / in_channels as f32);
					}
				}

				// Encode all complete 20 ms frames from the buffer.
				while buffer.len() >= FRAME_SAMPLES {
					let packet = encoder
						.encode(&buffer[..FRAME_SAMPLES], FRAME_SAMPLES)
						.context("failed to encode Opus frame")?;
					total_bytes += packet.len();
					packets.push(packet);
					buffer.drain(..FRAME_SAMPLES);
				}
			}
			Err(mpsc::RecvTimeoutError::Timeout) => continue,
			Err(mpsc::RecvTimeoutError::Disconnected) => break,
		}
	}

	drop(in_stream);

	let frames_encoded = packets.len();
	let avg_bytes = if frames_encoded > 0 {
		total_bytes as f64 / frames_encoded as f64
	} else {
		0.0
	};
	println!(
		"Encoded: {frames_encoded} frames ({} ms), {total_bytes} bytes total, {avg_bytes:.1} \
		 bytes/packet avg",
		frames_encoded * 20,
	);

	// ── Decode ────────────────────────────────────────────────────────────────
	println!("Decoding...");
	let mut decoder =
		OpusDecoder::new(SAMPLE_RATE as i32, 1).context("failed to create Opus decoder")?;
	let mut decoded = Vec::<f32>::with_capacity(frames_encoded * FRAME_SAMPLES);
	for packet in &packets {
		let frame = decoder
			.decode_float(Some(packet.as_slice()), FRAME_SAMPLES, false)
			.context("failed to decode Opus frame")?;
		decoded.extend_from_slice(&frame);
	}

	// ── Output setup ──────────────────────────────────────────────────────────
	let out_device = host
		.default_output_device()
		.context("no default output device found")?;
	println!(
		"Output device: {}",
		out_device
			.description()
			.map(|d| d.name().to_string())
			.unwrap_or_else(|_| "unknown".into())
	);

	let out_supported = out_device
		.supported_output_configs()
		.context("failed to query output configs")?
		.filter(|c| {
			c.sample_format() == SampleFormat::F32
				&& c.min_sample_rate() <= SAMPLE_RATE
				&& c.max_sample_rate() >= SAMPLE_RATE
		})
		.min_by_key(|c| c.channels())
		.context("output device does not support f32 at 48 kHz")?
		.with_sample_rate(SAMPLE_RATE);

	let out_channels = out_supported.channels() as usize;
	let out_config: StreamConfig = out_supported.into();

	// ── Playback ──────────────────────────────────────────────────────────────
	let samples = Arc::new(decoded);
	let pos = Arc::new(AtomicUsize::new(0));
	let (samples_cb, pos_cb) = (samples.clone(), pos.clone());

	let out_stream = out_device.build_output_stream(
		&out_config,
		move |data: &mut [f32], _| {
			let p = pos_cb.fetch_add(data.len() / out_channels, Ordering::Relaxed);
			for (i, frame) in data.chunks_mut(out_channels).enumerate() {
				// Expand mono sample to all output channels.
				let sample = samples_cb.get(p + i).copied().unwrap_or(0.0);
				frame.fill(sample);
			}
		},
		|err| eprintln!("output stream error: {err}"),
		None,
	)?;
	out_stream.play().context("failed to start output stream")?;

	let playback_duration = Duration::from_millis(frames_encoded as u64 * 20 + 100);
	println!("Playing back ({} ms)...", playback_duration.as_millis());
	std::thread::sleep(playback_duration);

	drop(out_stream);
	println!("Done.");

	Ok(())
}
