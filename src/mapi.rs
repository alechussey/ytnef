use crate::utils::*;

pub struct MAPIProperty {
	inner: ytnef_sys::MAPIProperty
}

impl MAPIProperty {
	/// # Safety
	///
	/// 'raw' must be a non-NULL initialized Attachment object. Only pass pointers returned by the
	/// ytnef_sys API to this function.
	pub unsafe fn from_raw(raw: *mut ytnef_sys::MAPIProperty) -> Self {
		Self { inner: raw.read() }
	}

	pub fn custom(&self) -> u32 {
		self.inner.custom
	}

	pub fn guid(&self) -> &[u8] {
		&self.inner.guid
	}

	pub fn id(&self) -> u32 {
		self.inner.id
	}

	pub fn count(&self) -> u32 {
		self.inner.count
	}

	pub fn named_properties(&self) -> Option<Vec<String>> {
		// `namedproperty' actually holds the number of named properties
		// contained within `propnames' - so much for descriptive variables
		let count: i32 = self.inner.namedproperty;

		if count < 1 {
			return None;
		}

		let mut output: Vec<String> = vec![];

		for i in 0..count {
			// propnames are actually an array of variableLength structs
			// so we need to get each item using some pointer arithmetic
			let data = unsafe {
				*self.inner.propnames.offset(i as isize)
			};
			output.push(string_from_varlen(data)?);
		}

		Some(output)
	}

	pub fn data(&self) -> Option<Vec<u8>> {
		vec_from_varlen(unsafe { self.inner.data.read() })
	}
}
