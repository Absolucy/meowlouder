// SPDX-License-Identifier: MPL-2.0
use meowlouder_opus_sys::{
	opus_strerror, OPUS_ALLOC_FAIL, OPUS_BAD_ARG, OPUS_BUFFER_TOO_SMALL, OPUS_INTERNAL_ERROR,
	OPUS_INVALID_PACKET, OPUS_INVALID_STATE, OPUS_UNIMPLEMENTED,
};
use std::{
	ffi::CStr,
	fmt::{Display, Error as FmtError, Formatter},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum OpusErrorCode {
	BadArg = OPUS_BAD_ARG,
	BufferTooSmall = OPUS_BUFFER_TOO_SMALL,
	InternalError = OPUS_INTERNAL_ERROR,
	InvalidPacket = OPUS_INVALID_PACKET,
	Unimplemented = OPUS_UNIMPLEMENTED,
	InvalidState = OPUS_INVALID_STATE,
	AllocFail = OPUS_ALLOC_FAIL,
}

impl OpusErrorCode {
	pub fn description(self) -> &'static str {
		// SAFETY: All possible values for [OpusErrorCode] are valid inputs for
		// [opus_strerror]
		unsafe {
			CStr::from_ptr(opus_strerror(self as i32))
				.to_str()
				.unwrap_unchecked()
		}
	}

	pub(crate) fn from_errno(errno: i32) -> Self {
		match errno {
			OPUS_BAD_ARG => Self::BadArg,
			OPUS_BUFFER_TOO_SMALL => Self::BufferTooSmall,
			OPUS_INTERNAL_ERROR => Self::InternalError,
			OPUS_INVALID_PACKET => Self::InvalidPacket,
			OPUS_UNIMPLEMENTED => Self::Unimplemented,
			OPUS_INVALID_STATE => Self::InvalidState,
			OPUS_ALLOC_FAIL => Self::AllocFail,
			_ => unreachable!("invalid libopus error code ({errno})"),
		}
	}
}

impl Display for OpusErrorCode {
	fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
		write!(f, "{}", self.description())
	}
}

impl std::error::Error for OpusErrorCode {}

#[macro_export]
macro_rules! map_error {
	($x:expr) => {{
		let result = $x;
		match result {
			..0 => Err($crate::error::OpusErrorCode::from_errno(result)),
			0.. => Ok(result),
		}
	}};
	(&$var:ident, $x:expr) => {{
		map_error!($x).map(|_| $var)
	}};
	((), $x:expr) => {{
		map_error!($x).map(|_| ())
	}};
	($return_type:ty, $x:expr) => {
		map_error!($x).map(|value| value as $return_type)
	};
}
