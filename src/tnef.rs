
use std::fmt;
use std::io::Read;
use std::ops::Drop;
use std::ffi::CString;
use std::convert::From;
use std::mem::MaybeUninit;
use std::os::raw::{c_int, c_void};
use chrono::NaiveDateTime;
use crate::mapi::MAPIProperty;
use crate::utils::*;

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TNEFError {
	CannotInitData   = -1,
	NotTnefStream    = -2,
	ErrorReadingData = -3,
	NoKey            = -4,
	BadChecksum      = -5,
	ErrorInHandler   = -6,
	UnknownProperty  = -7,
	IncorrectSetup   = -8,
	UnknownError     = -9
}

/// Used for conveniently converting return values from ytnef_sys into an error type
impl From<i32> for TNEFError {
	fn from(other: i32) -> Self {
		match other {
			-1 => TNEFError::CannotInitData,
			-2 => TNEFError::NotTnefStream,
			-3 => TNEFError::ErrorReadingData,
			-4 => TNEFError::NoKey,
			-5 => TNEFError::BadChecksum,
			-6 => TNEFError::ErrorInHandler,
			-7 => TNEFError::UnknownProperty,
			-8 => TNEFError::IncorrectSetup,
			_  => TNEFError::UnknownError
		}
	}
}

impl fmt::Display for TNEFError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let msg = match self {
			TNEFError::CannotInitData   => "Cannot initialize data",
			TNEFError::NotTnefStream    => "Not a TNEF stream",
			TNEFError::ErrorReadingData => "Error reading data",
			TNEFError::NoKey            => "No key",
			TNEFError::BadChecksum      => "Bad checksum",
			TNEFError::ErrorInHandler   => "Error in I/O handler",
			TNEFError::UnknownProperty  => "Unkown property",
			TNEFError::IncorrectSetup   => "Incorrect setup",
			_ => "Unkown error"
		};
		write!(f, "{} ({})", msg, *self as u8)
	}
}

pub type TNEFResult<T> = Result<T, TNEFError>;

pub struct TNEFFile {
	inner: ytnef_sys::TNEFStruct
}

struct ReaderWrapper {
	inner: Box<dyn Read>
}

unsafe extern "C" fn tnef_io_open(_io: *mut ytnef_sys::_TNEFIOStruct) -> c_int {
	0
}

unsafe extern "C" fn tnef_io_read(
	io: *mut ytnef_sys::_TNEFIOStruct,
	size: c_int,
	count: c_int,
	dest: *mut c_void
) -> c_int {
	// extract our reader from the `data' field in our I/O struct
	let mut reader = Box::from_raw((*io).data as *mut ReaderWrapper);

	// allocate a buffer sufficient for the amount of data we will read
	let buffer_size: usize = (size * count) as usize;
	let mut buffer: Vec<u8> = vec![0; buffer_size];

	// read data from our reader and write data to the `dest' buffer
	let bytes_read: i32 = match reader.inner.read(&mut buffer) {
		Ok(bytes_read) => bytes_read as i32,
		Err(_) => -1
	};
	buffer.as_ptr().copy_to(dest as *mut u8, buffer_size);

	// turn our box back into a raw pointer to avoid double free then
	// return our result
	(*io).data = Box::into_raw(reader) as *mut c_void;
	bytes_read
}

unsafe extern "C" fn tnef_io_close(_io: *mut ytnef_sys::_TNEFIOStruct) -> c_int {
	0
}

impl TNEFFile {
	// impl with Read trait instead
	pub fn new<R: 'static + Read>(reader: R) -> TNEFResult<Self> {
		let reader_wrapper = Box::new(ReaderWrapper {
			inner: Box::new(reader)
		});

		// configure IO struct
		let io = ytnef_sys::_TNEFIOStruct {
			InitProc: Some(tnef_io_open),
			ReadProc: Some(tnef_io_read),
			CloseProc: Some(tnef_io_close),
			data: Box::into_raw(reader_wrapper) as *mut c_void
		};
		
		// initialize TNEF struct
		let mut inner = MaybeUninit::<ytnef_sys::TNEFStruct>::zeroed();

		let result: i32 = unsafe {
			let mut inner_ptr = inner.as_mut_ptr();
			ytnef_sys::TNEFInitialize(inner_ptr);
			(*inner_ptr).IO = io; // insert our custom I/O interface before parsing
			(*inner_ptr).Debug = 0;
			ytnef_sys::TNEFParse(inner_ptr)
		};

		if result < 0 {
			Err(result.into())
		} else {
			Ok(Self { inner: unsafe { inner.assume_init() }})
		}
	}

	pub fn from_file(path: String) -> TNEFResult<Self> {
		let mut inner = MaybeUninit::<ytnef_sys::TNEFStruct>::zeroed();

		let result: i32 = unsafe {
			let path_cstr = match CString::new(path) {
				Ok(path) => path.into_raw(),
				Err(_) => {
					return Err(TNEFError::CannotInitData);
				}
			};
			
			let inner_ptr = inner.as_mut_ptr();
			ytnef_sys::TNEFInitialize(inner_ptr);
			let result = ytnef_sys::TNEFParseFile(path_cstr, inner_ptr);
			let _ = CString::from_raw(path_cstr);
			
			result
		};

		if result < 0 {
			Err(result.into())
		} else {
			Ok(Self { inner: unsafe { inner.assume_init() } })
		}
	}

	// FIXME: it sucks that we need this mutable borrow
	pub fn from_buffer(buffer: &mut [u8]) -> TNEFResult<Self> {
		let mut inner = MaybeUninit::<ytnef_sys::TNEFStruct>::zeroed();

		let result: i32 = unsafe {
			let inner_ptr = inner.as_mut_ptr();

			#[cfg(target_pointer_width = "32")]
			let buffer_len = buffer.len() as i32;
			#[cfg(target_pointer_width = "64")]
			let buffer_len = buffer.len() as i64;

			ytnef_sys::TNEFInitialize(inner_ptr);
			ytnef_sys::TNEFParseMemory(
				buffer.as_mut_ptr(),
				buffer_len,
				inner_ptr
			)
		};

		if result < 0 {
			Err(result.into())
		} else {
			Ok(Self { inner: unsafe { inner.assume_init() } })
		}
	}

	pub fn version(&self) -> String {
		make_safe_string(self.inner.version.as_ptr())
	}

	pub fn from(&self) -> Option<String> {
		string_from_varlen(self.inner.from)
	}

	pub fn subject(&self) -> Option<String> {
		string_from_varlen(self.inner.subject)
	}

	pub fn date_sent(&self) -> NaiveDateTime {
		datetime_from_dtr(self.inner.dateSent)
	}
	
	pub fn date_received(&self) -> NaiveDateTime {
		datetime_from_dtr(self.inner.dateReceived)
	}

	pub fn date_modified(&self) -> NaiveDateTime {
		datetime_from_dtr(self.inner.dateModified)
	}

	pub fn date_start(&self) -> NaiveDateTime {
		datetime_from_dtr(self.inner.DateStart)
	}

	pub fn date_end(&self) -> NaiveDateTime {
		datetime_from_dtr(self.inner.DateEnd)
	}	

	pub fn message_status(&self) -> String {
		// FIXME: maybe make this an enum
		make_safe_string(self.inner.messageStatus.as_ptr())
	}

	pub fn message_class(&self) -> String {
		// FIXME: maybe make this an enum
		make_safe_string(self.inner.messageClass.as_ptr())
	}

	pub fn message_id(&self) -> String {
		make_safe_string(self.inner.messageID.as_ptr())
	}

	pub fn parent_id(&self) -> String {
		make_safe_string(self.inner.parentID.as_ptr())
	}

	pub fn conversation_id(&self) -> String {
		make_safe_string(self.inner.conversationID.as_ptr())
	}

	pub fn body(&self) -> Option<String> {
		string_from_varlen(self.inner.body)
	}

	pub fn priority(&self) -> String {
		// FIXME: make this an enum type
		make_safe_string(self.inner.priority.as_ptr())
	}

	pub fn attachments(&self) -> Vec<TNEFAttachment> {
		let mut output: Vec<TNEFAttachment> = vec![];
		let mut curr_attach = self.inner.starting_attach.next;

		// push starting attachment itself onto output
		output.push(TNEFAttachment { inner: self.inner.starting_attach });

		while !curr_attach.is_null() {
			let attach = unsafe { TNEFAttachment::from_raw(curr_attach) };
			curr_attach = attach.inner.next;
			output.push(attach);
		}

		output
	}

	pub fn mapi_properties(&self) -> Vec<MAPIProperty> {
		let mut output: Vec<MAPIProperty> = vec![];
		let props_list = self.inner.MapiProperties;

		for i in 0..props_list.count {
			let property = unsafe {
				MAPIProperty::from_raw(
					props_list.properties.offset(i as isize)
				)
			};
			output.push(property);
		}

		output
	}

	pub fn code_page(&self) -> Option<Vec<u8>> {
		vec_from_varlen(self.inner.CodePage)
	}

	pub fn original_message_class(&self) -> Option<String> {
		// FIXME: maybe make enum
		string_from_varlen(self.inner.OriginalMessageClass)
	}

	pub fn owner(&self) -> Option<String> {
		string_from_varlen(self.inner.Owner)
	}

	pub fn sent_for(&self) -> Option<String> {
		string_from_varlen(self.inner.SentFor)
	}

	pub fn delegate(&self) -> Option<String> {
		string_from_varlen(self.inner.Delegate)
	}

	pub fn aid_owner(&self) -> Option<String> {
		string_from_varlen(self.inner.AidOwner)
	}
}

impl Drop for TNEFFile {
	fn drop(&mut self) {
		unsafe { ytnef_sys::TNEFFree(&mut self.inner); }
	}
}

pub struct TNEFAttachmentRenderData {
	pub attach_type: u16,
	pub position: u32,
	pub width: u16,
	pub height: u16,
	pub flags: u32
}

// So this is my first attempt at creating a C interface library in Rust and
// I wasn't sure what to do with the linked-list pointers used in this structure.
// I tried the Box<T> method and realized that wouldn't work because you can't
// drop the value since drop() borrows self and you can't mutably borrow the inner
// value from the box. Just decided to copy the structures instead.
pub struct TNEFAttachment {
	inner: ytnef_sys::Attachment
}

impl Default for TNEFAttachment {
	fn default() -> Self {
		Self::new()
	}
}

impl TNEFAttachment {
	pub fn new() -> Self {
		unsafe {
			let mut inner = MaybeUninit::<ytnef_sys::Attachment>::zeroed();
			ytnef_sys::TNEFInitAttachment(inner.as_mut_ptr());
			Self { inner: inner.assume_init() }
		}
	}

	/// # Safety
	///
	/// 'raw' must be a non-NULL initialized Attachment object. Only pass pointers returned by the
	/// ytnef_sys API to this function.
	pub unsafe fn from_raw(raw: *mut ytnef_sys::Attachment) -> Self {
		Self { inner: raw.read() }
	}

	pub fn date(&self) -> NaiveDateTime {
		datetime_from_dtr(self.inner.Date)
	}

	pub fn title(&self) -> Option<String> {
		string_from_varlen(self.inner.Title)
	}

	pub fn create_date(&self) -> NaiveDateTime {
		datetime_from_dtr(self.inner.CreateDate)
	}

	pub fn modify_date(&self) -> NaiveDateTime {
		datetime_from_dtr(self.inner.ModifyDate)
	}

	pub fn transport_filename(&self) -> Option<String> {
		string_from_varlen(self.inner.TransportFilename)
	}

	pub fn render_data(&self) -> TNEFAttachmentRenderData {
		TNEFAttachmentRenderData {
			attach_type: self.inner.RenderData.atyp,
			position: self.inner.RenderData.ulPosition,
			width:    self.inner.RenderData.dxWidth,
			height:   self.inner.RenderData.dyHeight,
			flags:    self.inner.RenderData.dwFlags
		}
	}

	pub fn file_data(&self) -> Option<Vec<u8>> {
		vec_from_varlen(self.inner.FileData)
	}

	pub fn icon_data(&self) -> Option<Vec<u8>> {
		vec_from_varlen(self.inner.IconData)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use std::fs::{read, File};

	#[test]
	fn new_from_file() {
		let file = File::open("test_data/winmail.dat").unwrap();
		let _ = TNEFFile::new(file).unwrap();
	}

	#[test]
	fn new_from_path() {
		let _ = TNEFFile::from_file("test_data/winmail.dat".to_string()).unwrap();
	}

	#[test]
	fn new_from_buffer() {
		let mut buffer: Vec<u8> = read("test_data/winmail.dat").unwrap();
		let _ = TNEFFile::from_buffer(&mut buffer).unwrap();
	}
}
