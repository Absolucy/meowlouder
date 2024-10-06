// SPDX-License-Identifier: MPL-2.0
use crate::{error::OpusErrorCode, map_error};
use meowlouder_opus_sys::{
	opus_decode, opus_decode_float, opus_decoder_ctl, opus_decoder_get_size, opus_decoder_init,
	OPUS_GET_BANDWIDTH_REQUEST, OPUS_GET_LAST_PACKET_DURATION_REQUEST, OPUS_GET_PITCH_REQUEST,
	OPUS_GET_SAMPLE_RATE_REQUEST, OPUS_RESET_STATE,
};

#[derive(Clone)]
pub struct OpusDecoder {
	decoder_state: Box<[u8]>,
	channels: usize,
}

impl OpusDecoder {
	pub fn new(sample_rate: i32, channels: i32) -> Result<Self, OpusErrorCode> {
		debug_assert!(channels <= 2, "channels cannot be over 2");
		let decoder_size = unsafe { opus_decoder_get_size(channels) as usize };
		let mut decoder_state = vec![0u8; decoder_size].into_boxed_slice();
		map_error!(unsafe {
			opus_decoder_init(decoder_state.as_mut_ptr().cast(), sample_rate, channels)
		})?;
		Ok(Self {
			decoder_state,
			channels: channels as usize,
		})
	}

	pub fn decode_into<Data, Pcm>(
		&mut self,
		data: Option<Data>,
		mut pcm: Pcm,
		frame_size: usize,
		decode_fec: bool,
	) -> Result<usize, OpusErrorCode>
	where
		Data: AsRef<[u8]>,
		Pcm: AsMut<[i16]>,
	{
		let pcm = pcm.as_mut();
		if !cfg!(feature = "i-can-be-trusted-to-size-my-decoder-buffer-correctly")
			&& pcm.len() < frame_size * self.channels
		{
			return Err(OpusErrorCode::BufferTooSmall);
		}

		let (data_ptr, data_len) = data
			.as_ref()
			.map(|d| {
				let data = d.as_ref();
				(data.as_ptr(), data.len() as i32)
			})
			.unwrap_or((std::ptr::null(), 0));

		map_error!(usize, unsafe {
			opus_decode(
				self.decoder_state.as_mut_ptr().cast(),
				data_ptr,
				data_len,
				pcm.as_mut_ptr(),
				frame_size as _,
				decode_fec as _,
			)
		})
	}

	pub fn decode<Data>(
		&mut self,
		data: Option<Data>,
		frame_size: usize,
		decode_fec: bool,
	) -> Result<Vec<i16>, OpusErrorCode>
	where
		Data: AsRef<[u8]>,
	{
		let mut pcm = vec![0; frame_size * self.channels];
		let len = self.decode_into(data, &mut pcm, frame_size, decode_fec)?;
		pcm.truncate(len);
		Ok(pcm)
	}

	pub fn decode_float_into<Data, Pcm>(
		&mut self,
		data: Option<Data>,
		mut pcm: Pcm,
		frame_size: usize,
		decode_fec: bool,
	) -> Result<usize, OpusErrorCode>
	where
		Data: AsRef<[u8]>,
		Pcm: AsMut<[f32]>,
	{
		let pcm = pcm.as_mut();

		if !cfg!(feature = "i-can-be-trusted-to-size-my-decoder-buffer-correctly")
			&& pcm.len() < frame_size * self.channels
		{
			return Err(OpusErrorCode::BufferTooSmall);
		}

		let (data_ptr, data_len) = data
			.as_ref()
			.map(|d| {
				let data = d.as_ref();
				(data.as_ptr(), data.len() as i32)
			})
			.unwrap_or((std::ptr::null(), 0));

		map_error!(usize, unsafe {
			opus_decode_float(
				self.decoder_state.as_mut_ptr().cast(),
				data_ptr,
				data_len,
				pcm.as_mut_ptr(),
				frame_size as _,
				decode_fec as _,
			)
		})
	}

	pub fn decode_float<Data>(
		&mut self,
		data: Option<Data>,
		frame_size: usize,
		decode_fec: bool,
	) -> Result<Vec<f32>, OpusErrorCode>
	where
		Data: AsRef<[u8]>,
	{
		let mut pcm = vec![0.0; frame_size * self.channels];
		let len = self.decode_float_into(data, &mut pcm, frame_size, decode_fec)?;
		pcm.truncate(len);
		Ok(pcm)
	}

	/// Resets the codec state to be equivalent to a freshly initialized state.
	/// This should be called when switching streams in order to prevent the
	/// back to back decoding from giving different results from one at a time
	/// decoding.
	pub fn reset(&mut self) -> Result<(), OpusErrorCode> {
		map_error!((), unsafe {
			opus_decoder_ctl(
				self.decoder_state.as_mut_ptr().cast(),
				OPUS_RESET_STATE as _,
			)
		})?;
		Ok(())
	}

	/// Returns the decoder's last bandpass.
	pub fn bandwidth(&mut self) -> Result<i32, OpusErrorCode> {
		let mut bandwidth = 0;
		map_error!(&bandwidth, unsafe {
			opus_decoder_ctl(
				self.decoder_state.as_mut_ptr().cast(),
				OPUS_GET_BANDWIDTH_REQUEST as _,
				&mut bandwidth,
			)
		})
	}

	/// Returns the sampling rate the decoder was initialized with.
	pub fn sample_rate(&mut self) -> Result<i32, OpusErrorCode> {
		let mut sample_rate = 0;
		map_error!(&sample_rate, unsafe {
			opus_decoder_ctl(
				self.decoder_state.as_mut_ptr().cast(),
				OPUS_GET_SAMPLE_RATE_REQUEST as _,
				&mut sample_rate,
			)
		})
	}

	/// Returns the duration (in samples, at the current sampling rate) of the
	/// last packet successfully decoded or concealed.
	pub fn last_packet_duration(&mut self) -> Result<i32, OpusErrorCode> {
		let mut packet_duration = 0;
		map_error!(&packet_duration, unsafe {
			opus_decoder_ctl(
				self.decoder_state.as_mut_ptr().cast(),
				OPUS_GET_LAST_PACKET_DURATION_REQUEST as _,
				&mut packet_duration,
			)
		})
	}

	/// Returns the pitch period (at 48 kHz) of the last decoded frame, if
	/// available. This can be used for any post-processing algorithm requiring
	/// the use of pitch, e.g. time stretching/shortening. If the last frame
	/// was not voiced, or if the pitch was not coded in the frame, then zero
	/// is returned.
	pub fn pitch(&mut self) -> Result<Option<i32>, OpusErrorCode> {
		let mut pitch = 0;
		map_error!(unsafe {
			opus_decoder_ctl(
				self.decoder_state.as_mut_ptr().cast(),
				OPUS_GET_PITCH_REQUEST as _,
				&mut pitch,
			)
		})
		.map(|pitch| if pitch == 0 { None } else { Some(pitch) })
	}
}
