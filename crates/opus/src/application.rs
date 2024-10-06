// SPDX-License-Identifier: MPL-2.0
use meowlouder_opus_sys::{
	OPUS_APPLICATION_AUDIO, OPUS_APPLICATION_RESTRICTED_LOWDELAY, OPUS_APPLICATION_VOIP,
};

/// The coding mode for an Opus encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum OpusApplication {
	/// Best for most VoIP/videoconference applications where listening quality
	/// and intelligibility matter most.
	Voip = OPUS_APPLICATION_VOIP,
	/// Best for broadcast/high-fidelity application where the decoded audio
	/// should be as close as possible to the input.
	Audio = OPUS_APPLICATION_AUDIO,
	/// Only use when lowest-achievable latency is what matters most.
	/// Voice-optimized modes cannot be used.
	RestrictedLowDelay = OPUS_APPLICATION_RESTRICTED_LOWDELAY,
}

impl From<OpusApplication> for u32 {
	fn from(value: OpusApplication) -> Self {
		value as u32
	}
}

impl From<OpusApplication> for i32 {
	fn from(value: OpusApplication) -> Self {
		value as i32
	}
}
