// SPDX-License-Identifier: MPL-2.0
use crate::{application::OpusApplication, encode::OpusEncodable, error::OpusErrorCode, map_error};
use meowlouder_opus_sys::{
	opus_encoder_ctl, opus_encoder_get_size, opus_encoder_init, OPUS_GET_BANDWIDTH_REQUEST,
	OPUS_GET_PACKET_LOSS_PERC_REQUEST, OPUS_GET_SAMPLE_RATE_REQUEST, OPUS_RESET_STATE,
	OPUS_SET_PACKET_LOSS_PERC_REQUEST,
};

const MAX_DATA_BYTES: usize = 1275;

#[derive(Clone)]
pub struct OpusEncoder {
	pub(crate) encoder_state: Box<[u8]>,
}

impl OpusEncoder {
	pub fn new(
		sample_rate: i32,
		channels: i32,
		application: OpusApplication,
	) -> Result<Self, OpusErrorCode> {
		debug_assert!(channels <= 2, "channels cannot be over 2");
		let encoder_size = unsafe { opus_encoder_get_size(channels) as usize };
		let mut encoder_state = vec![0; encoder_size].into_boxed_slice();
		map_error!(unsafe {
			opus_encoder_init(
				encoder_state.as_mut_ptr().cast(),
				sample_rate,
				channels,
				application.into(),
			)
		})?;
		Ok(Self { encoder_state })
	}

	pub fn encode_into<T: OpusEncodable>(
		&mut self,
		pcm: &[T],
		frame_size: usize,
		data: &mut [u8],
	) -> Result<usize, OpusErrorCode> {
		T::encode(self, pcm, frame_size, data)
	}

	pub fn encode<T: OpusEncodable>(
		&mut self,
		pcm: &[T],
		frame_size: usize,
	) -> Result<Vec<u8>, OpusErrorCode> {
		let mut data = vec![0; MAX_DATA_BYTES];
		let len = self.encode_into(pcm, frame_size, &mut data)?;
		data.truncate(len);
		Ok(data)
	}

	/// Resets the codec state to be equivalent to a freshly initialized state.
	/// This should be called when switching streams in order to prevent the
	/// back to back decoding from giving different results from one at a time
	/// decoding.
	pub fn reset(&mut self) -> Result<(), OpusErrorCode> {
		map_error!((), unsafe {
			opus_encoder_ctl(
				self.encoder_state.as_mut_ptr().cast(),
				OPUS_RESET_STATE as _,
			)
		})
	}

	/// Returns the encoder's configured bandpass.
	pub fn bandwidth(&mut self) -> Result<i32, OpusErrorCode> {
		let mut bandwidth = 0;
		map_error!(&bandwidth, unsafe {
			opus_encoder_ctl(
				self.encoder_state.as_mut_ptr().cast(),
				OPUS_GET_BANDWIDTH_REQUEST as _,
				&mut bandwidth,
			)
		})
	}

	/// Returns the sampling rate the encoder was initialized with.
	pub fn sample_rate(&mut self) -> Result<i32, OpusErrorCode> {
		let mut sample_rate = 0;
		map_error!(&sample_rate, unsafe {
			opus_encoder_ctl(
				self.encoder_state.as_mut_ptr().cast(),
				OPUS_GET_SAMPLE_RATE_REQUEST as _,
				&mut sample_rate,
			)
		})
	}

	/// Returns the encoder's configured packet loss percentage
	/// in the range of 0-100, include (default: 0).
	pub fn expected_packet_loss(&mut self) -> Result<i32, OpusErrorCode> {
		let mut packet_loss_percent = 0;
		map_error!(&packet_loss_percent, unsafe {
			opus_encoder_ctl(
				self.encoder_state.as_mut_ptr().cast(),
				OPUS_GET_PACKET_LOSS_PERC_REQUEST as _,
				&mut packet_loss_percent,
			)
		})
	}

	/// Configures the encoder's expected packet loss percentage.
	/// Higher values trigger progressively more loss resistant behavior in the
	/// encoder at the expense of quality at a given bitrate in the absence of
	/// packet loss, but greater quality under loss.
	///
	/// `percentage` is the loss percentage in range 0-100, inclusive (default:
	/// 0).
	pub fn set_expected_packet_loss(&mut self, percentage: i32) -> Result<(), OpusErrorCode> {
		map_error!((), unsafe {
			opus_encoder_ctl(
				self.encoder_state.as_mut_ptr().cast(),
				OPUS_SET_PACKET_LOSS_PERC_REQUEST as _,
				percentage,
			)
		})
	}
}
