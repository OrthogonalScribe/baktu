pub mod tagged_rawhex {
	// TODO: (C) consider using Cow
	/// Encodes a byte sequence into either a raw, or a lowercase hex variant, prefixed with
	/// `r-<size in bytes> ` or  `h ` respectively.
	///
	/// Does hex if the sequence contains `b'\n'` or additionally `b' '`.
	///
	/// The `<size in bytes>` added to the raw tag is to make it harder for the user to miss (or an
	/// editor to mangle) values that end on non-printing characters, for example the
	/// `security.capabilities` extended attribute.
	pub fn encode(hex_on_space: bool, bytes: &[u8]) -> Vec<u8> {
		let mut raw = true;
		for byte in bytes {
			if *byte == b'\n' || hex_on_space && *byte == b' ' {
				raw = false;
			}
		}

		let mut result = if raw {
			format!("r-{} ", bytes.len()).as_bytes().into()
		} else {
			vec![b'h', b' ']
		};

		if raw {
			result.extend(bytes)
		} else {
			result.extend(super::encode(bytes))
		};

		result
	}
}

/// Produces lowercase-hex encoded data
fn encode(bytes: &[u8]) -> Vec<u8> {
	let mut result = Vec::new();
	let hex_char = b"0123456789abcdef";
	for byte in bytes {
		result.push(hex_char[(*byte >> 4) as usize]);
		result.push(hex_char[(*byte & 0x0F) as usize]);
	}
	result
}

/// Decodes hex encoded data, assumes even length of input
pub fn decode(hex_str: &[u8]) -> Vec<u8> {
	// TODO: (C) add capitals, switch to result, as we want to have more options than panic on a
	// corrupt meta file
	assert!(hex_str.len() % 2 == 0);

	fn val_of(c: u8) -> u8 {
		match c {
			b'0'..=b'9' => c - b'0',
			b'a'..=b'f' => c - b'a' + 10,
			// assume no invalid values, as we're specialized for `get-all-xattrs` output
			c => panic!("unexpected value {c} in hex input"),
		}
	}

	hex_str
		.chunks(2)
		.map(|pair| val_of(pair[0]) << 4 | val_of(pair[1]))
		.collect()
}
