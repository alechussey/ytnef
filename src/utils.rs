use std::slice;
use std::os::raw::c_char;
use std::ffi::CStr;
use chrono::{NaiveDate, NaiveTime, NaiveDateTime};

/// Conveniently convert a C-string to a Rust string.
pub fn make_safe_string(data: *const c_char) -> String {
	unsafe {
		CStr::from_ptr(data)
			.to_string_lossy()
			.to_string()
	}
}

/// Convenient method for converting variableLength to Vec<u8>
pub fn vec_from_varlen(
	attr: ytnef_sys::variableLength
) -> Option<Vec<u8>> {
	if attr.size < 1 {
		return None;
	}

	Some(unsafe {
		slice::from_raw_parts(attr.data, attr.size as usize)
			.to_vec()
	})
}

/// Conveniently convert the variableLength native type from the ytnef library
/// to a standard Rust string.
pub fn string_from_varlen(
	attr: ytnef_sys::variableLength
) -> Option<String> {
	String::from_utf8(vec_from_varlen(attr)?).ok()
}

/// Conveniently convertthe dtr native type fro ytnef to chrono::NaiveDateTime
pub fn datetime_from_dtr(date: ytnef_sys::dtr) -> NaiveDateTime {
	let ndate = NaiveDate::from_ymd(
		date.wYear as i32,
		date.wMonth as u32,
		date.wDay as u32
	);

	let ntime = NaiveTime::from_hms(
		date.wHour as u32,
		date.wMinute as u32,
		date.wSecond as u32
	);

	NaiveDateTime::new(ndate, ntime)
}
