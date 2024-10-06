use color_eyre::eyre::{ContextCompat, Result, WrapErr};
use cpal::{
	traits::{DeviceTrait, HostTrait, StreamTrait},
	Sample, SampleFormat,
};
use crossbeam_channel::{Receiver, Sender};
use meowlouder_opus::{OpusApplication, OpusEncoder};

fn main() -> Result<()> {
	color_eyre::install()?;
	let host = cpal::default_host();

	for device in host
		.input_devices()
		.wrap_err("failed to get input devices")?
	{
		let device_name = device.name().wrap_err("failed to get device name")?;
		println!("Device name: {device_name}");

		for (idx, config) in device
			.supported_input_configs()
			.wrap_err("failed to get supported input configs")?
			.enumerate()
		{
			println!("input config #{idx}: {config:?}");
		}
	}

	/*
	let default_config = device
		.default_input_config()
		.wrap_err("failed to get default input config")?;

		println!("Default input config: {default_config:?}");

		let config =

		let sample_rate = device_config.sample_rate();
		let channels = device_config.channels();

		let mut encoder = OpusEncoder::new(
			sample_rate.0 as i32,
			channels as i32,
			OpusApplication::Audio,
		)
		.wrap_err("failed to create opus encoder")?;
	*/

	Ok(())
}
