// SPDX-License-Identifier: MPL-2.0
use crate::{encode::OpusEncoder, error::OpusErrorCode, map_error};
use meowlouder_opus_sys::{opus_encode, opus_encode_float};

pub trait OpusEncodable: Sized {
	fn encode(
		encoder: &mut OpusEncoder,
		pcm: &[Self],
		frame_size: usize,
		data: &mut [u8],
	) -> Result<usize, OpusErrorCode>;
}

impl OpusEncodable for i16 {
	fn encode(
		encoder: &mut OpusEncoder,
		pcm: &[Self],
		frame_size: usize,
		data: &mut [u8],
	) -> Result<usize, OpusErrorCode> {
		map_error!(usize, unsafe {
			opus_encode(
				encoder.encoder_state.as_mut_ptr().cast(),
				pcm.as_ptr(),
				frame_size as _,
				data.as_mut_ptr(),
				data.len() as _,
			)
		})
	}
}

impl OpusEncodable for f32 {
	fn encode(
		encoder: &mut OpusEncoder,
		pcm: &[Self],
		frame_size: usize,
		data: &mut [u8],
	) -> Result<usize, OpusErrorCode> {
		map_error!(usize, unsafe {
			opus_encode_float(
				encoder.encoder_state.as_mut_ptr().cast(),
				pcm.as_ptr(),
				frame_size as _,
				data.as_mut_ptr(),
				data.len() as _,
			)
		})
	}
}
