// SPDX-License-Identifier: MPL-2.0
#![warn(
	clippy::correctness,
	clippy::suspicious,
	clippy::complexity,
	clippy::perf,
	clippy::style
)]
pub mod application;
pub mod decoder;
pub mod encode;
#[macro_use]
pub mod error;

pub use crate::{
	application::OpusApplication,
	decoder::OpusDecoder,
	encode::{OpusEncodable, OpusEncoder},
};

/// Returns the libopus version string.
///
/// Applications may look for the substring "-fixed" in the version string to
/// determine whether they have a fixed-point or floating-point build at
/// runtime.
pub fn libopus_version() -> &'static str {
	use meowlouder_opus_sys::opus_get_version_string;
	use std::ffi::CStr;

	// SAFETY: the libopus version string is guaranteed to be a valid string.
	unsafe {
		CStr::from_ptr(opus_get_version_string())
			.to_str()
			.unwrap_unchecked()
	}
}
