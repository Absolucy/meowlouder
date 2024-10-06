use anyhow::Result;
use cpal::{
	traits::{DeviceTrait, HostTrait, StreamTrait},
	Sample, SampleFormat, StreamConfig,
};
use crossbeam_channel::{select, Receiver, Sender};
use meowlouder_opus::{OpusApplication, OpusDecoder, OpusEncoder};
use rubato::{InterpolationParameters, InterpolationType, Resampler, SincFixedIn, WindowFunction};
use std::{io::BufRead, time::Duration};

struct EncodedChunk {
	data: Vec<u8>,
	frame_size: usize,
}

struct AudioBuffer {
	data: Vec<i16>,
	channels: u16,
	position: usize,
}

impl AudioBuffer {
	fn new(data: Vec<i16>, channels: u16) -> Self {
		Self {
			data,
			channels,
			position: 0,
		}
	}

	fn get_next_chunk(&mut self, chunk_size: usize) -> Option<&[i16]> {
		if self.position >= self.data.len() {
			return None;
		}

		let remaining = self.data.len() - self.position;
		let chunk_samples = chunk_size * self.channels as usize;
		let take_samples = chunk_samples.min(remaining);

		let chunk = &self.data[self.position..self.position + take_samples];
		self.position += take_samples;

		Some(chunk)
	}
}

fn create_resampler(
	from_sample_rate: u32,
	to_sample_rate: u32,
	channels: usize,
) -> Result<SincFixedIn<f32>> {
	let params = InterpolationParameters {
		sinc_len: 256,
		f_cutoff: 0.95,
		interpolation: InterpolationType::Linear,
		oversampling_factor: 256,
		window: WindowFunction::BlackmanHarris2,
	};

	Ok(SincFixedIn::<f32>::new(
		from_sample_rate as f64 / to_sample_rate as f64,
		params,
		channels,
		4096,
	)?)
}

fn resample_audio(
	input: &[i16],
	channels: usize,
	from_rate: u32,
	to_rate: u32,
	resampler: &mut SincFixedIn<f32>,
) -> Vec<i16> {
	// Convert i16 to f32
	let mut input_f32: Vec<Vec<f32>> = vec![Vec::new(); channels];
	for (i, sample) in input.iter().enumerate() {
		input_f32[i % channels].push(*sample as f32 / 32768.0);
	}

	// Resample
	let output_f32 = resampler.process(&input_f32, None).unwrap_or_default();

	// Convert back to interleaved i16
	let mut output = Vec::with_capacity(output_f32[0].len() * channels);
	for i in 0..output_f32[0].len() {
		for channel in 0..channels {
			output.push((output_f32[channel][i] * 32768.0) as i16);
		}
	}

	output
}

fn main() -> Result<()> {
	// Initialize the default host and devices
	let host = cpal::default_host();
	let input_device = host
		.default_input_device()
		.expect("no input device available");
	let output_device = host
		.default_output_device()
		.expect("no output device available");

	// Get the default configs
	let input_config = input_device.default_input_config()?;
	println!("Input config: {:?}", input_config);

	let output_config = output_device.default_output_config()?;
	println!("Output config: {:?}", output_config);

	let sample_rate = input_config.sample_rate().0;
	let channels = input_config.channels();

	// Calculate frame size based on desired duration
	let chunk_duration = Duration::from_millis(40);
	let samples_per_chunk =
		(sample_rate as u128 * chunk_duration.as_nanos() / 1_000_000_000) as usize;
	println!(
		"Using {} samples per chunk ({:?})",
		samples_per_chunk, chunk_duration
	);

	// Create channels for the audio data
	let (tx, rx) = crossbeam_channel::unbounded();
	let (finish_tx, finish_rx) = crossbeam_channel::unbounded::<()>();
	//let (encoded_tx, encoded_rx) = mpsc::channel();

	// Create encoder and decoder
	let mut encoder = OpusEncoder::new(
		sample_rate as i32,
		channels.min(2) as i32,
		OpusApplication::Audio,
	)?;

	// Set up the audio input stream
	let stream = match input_config.sample_format() {
		SampleFormat::F32 => input_device.build_input_stream(
			&input_config.into(),
			move |data: &[f32], _: &_| handle_input_data_f32(data, &tx, channels),
			err_fn,
			None,
		)?,
		SampleFormat::I16 => input_device.build_input_stream(
			&input_config.into(),
			move |data: &[i16], _: &_| handle_input_data_i16(data, &tx, channels),
			err_fn,
			None,
		)?,
		SampleFormat::U16 => input_device.build_input_stream(
			&input_config.into(),
			move |data: &[u16], _: &_| {
				let i16_data: Vec<i16> = data.iter().map(|&x| (x as i32 - 32768) as i16).collect();
				handle_input_data_i16(&i16_data, &tx, channels);
			},
			err_fn,
			None,
		)?,
		_ => unimplemented!(),
	};

	stream.play()?;

	// Spawn a thread to handle user input for stopping
	std::thread::spawn(move || {
		println!("Recording... Press Enter to stop and play back");
		let mut stdin = std::io::stdin().lock();
		let _ = stdin.read_line(&mut String::new());
		println!("okey done");
		let _ = finish_tx.send(());
	});

	// Buffer for accumulating samples
	let mut sample_buffer = Vec::new();
	let mut encoded_chunks = Vec::new();

	// Process incoming audio data in chunks
	loop {
		let data = select! {
			recv(rx) -> msg => match msg {
				Ok(data) => data,
				_ => break,
			},
			recv(finish_rx) -> _ => break,
		};
		sample_buffer.extend(data);

		// Process complete chunks
		while sample_buffer.len() >= samples_per_chunk * channels as usize {
			let chunk: Vec<i16> = sample_buffer
				.drain(..samples_per_chunk * channels as usize)
				.collect();

			match encoder.encode(&chunk, samples_per_chunk) {
				Ok(encoded) => {
					println!("Encoded chunk of {} bytes", encoded.len());
					encoded_chunks.push(EncodedChunk {
						data: encoded,
						frame_size: samples_per_chunk,
					});
				}
				Err(e) => eprintln!("Encoding error: {}", e),
			}
		}
	}

	// Process any remaining samples
	if !sample_buffer.is_empty() {
		let frame_size = sample_buffer.len() / channels as usize;
		match encoder.encode(&sample_buffer, frame_size) {
			Ok(encoded) => {
				println!("Encoded final chunk of {} bytes", encoded.len());
				encoded_chunks.push(EncodedChunk {
					data: encoded,
					frame_size,
				});
			}
			Err(e) => eprintln!("Encoding error: {}", e),
		}
	}

	println!("Playing back recorded audio...");

	// Create decoder for playback
	let mut decoder = OpusDecoder::new(sample_rate as i32, channels.min(2) as i32)?;

	// Set up output stream
	let (playback_tx, playback_rx) = crossbeam_channel::unbounded();

	let output_stream = match output_config.sample_format() {
		SampleFormat::F32 => output_device.build_output_stream(
			&output_config.into(),
			move |data: &mut [f32], _: &_| handle_output_data_f32(data, &playback_rx),
			err_fn,
			None,
		)?,
		SampleFormat::I16 => output_device.build_output_stream(
			&output_config.into(),
			move |data: &mut [i16], _: &_| handle_output_data_i16(data, &playback_rx),
			err_fn,
			None,
		)?,
		SampleFormat::U16 => output_device.build_output_stream(
			&output_config.into(),
			move |data: &mut [u16], _: &_| handle_output_data_u16(data, &playback_rx),
			err_fn,
			None,
		)?,
		_ => unimplemented!(),
	};

	output_stream.play()?;

	// Decode and play back each chunk
	for chunk in encoded_chunks {
		match decoder.decode(Some(&chunk.data), chunk.frame_size, false) {
			Ok(decoded) => {
				playback_tx.send(decoded)?;
			}
			Err(e) => eprintln!("Decoding error: {}", e),
		}
	}

	// Wait a bit for the last samples to play
	std::thread::sleep(Duration::from_secs(1));

	Ok(())
}

fn handle_output_data_f32(output: &mut [f32], rx: &Receiver<Vec<i16>>) {
	if let Ok(data) = rx.try_recv() {
		for (i, sample) in data.iter().enumerate() {
			if i < output.len() {
				output[i] = *sample as f32 / 32768.0;
			}
		}
	}
}

fn handle_output_data_i16(output: &mut [i16], rx: &Receiver<Vec<i16>>) {
	if let Ok(data) = rx.try_recv() {
		for (i, sample) in data.iter().enumerate() {
			if i < output.len() {
				output[i] = *sample;
			}
		}
	}
}

fn handle_output_data_u16(output: &mut [u16], rx: &Receiver<Vec<i16>>) {
	if let Ok(data) = rx.try_recv() {
		for (i, sample) in data.iter().enumerate() {
			if i < output.len() {
				output[i] = (*sample as i32 + 32768) as u16;
			}
		}
	}
}

// Previous input handling functions remain the same
fn handle_input_data_f32(input: &[f32], tx: &Sender<Vec<i16>>, channels: u16) {
	let mut processed: Vec<i16> = Vec::with_capacity(input.len());

	if channels <= 2 {
		for &sample in input {
			processed.push((sample * 32767.0) as i16);
		}
	} else {
		for chunk in input.chunks(channels as usize) {
			let mut left = 0.0;
			let mut right = 0.0;

			for (i, &sample) in chunk.iter().enumerate() {
				if i % 2 == 0 {
					left += sample;
				} else {
					right += sample;
				}
			}

			left /= channels as f32 / 2.0;
			right /= channels as f32 / 2.0;

			processed.push((left * 32767.0) as i16);
			processed.push((right * 32767.0) as i16);
		}
	}

	tx.send(processed).unwrap_or_default();
}

fn handle_input_data_i16(input: &[i16], tx: &Sender<Vec<i16>>, channels: u16) {
	if channels <= 2 {
		tx.send(input.to_vec()).unwrap_or_default();
	} else {
		let mut processed: Vec<i16> = Vec::with_capacity(input.len() * 2 / channels as usize);

		for chunk in input.chunks(channels as usize) {
			let mut left = 0i32;
			let mut right = 0i32;

			for (i, &sample) in chunk.iter().enumerate() {
				if i % 2 == 0 {
					left += sample as i32;
				} else {
					right += sample as i32;
				}
			}

			left /= channels as i32 / 2;
			right /= channels as i32 / 2;

			processed.push(left as i16);
			processed.push(right as i16);
		}

		tx.send(processed).unwrap_or_default();
	}
}

fn err_fn(err: cpal::StreamError) {
	eprintln!("an error occurred on stream: {}", err);
}
