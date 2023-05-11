use analyzer_abstractions::lsp_types::{self, Position, TextDocumentContentChangeEvent};

#[derive(Clone, Debug)]
pub struct LspPos {
	eof: bool, // flag indicates if the last character in file is '\n'
	ranges: Vec<usize>,
}

impl LspPos {
	// helper function
	fn parse_string(string: &String) -> (Vec<usize>, bool) {
		let mut result: Vec<usize> = Vec::new();
		if string.is_empty() {
			return (result, false);
		}

		let bytes = string.as_bytes();
		for i in 0..bytes.len() {
			if bytes[i] == b'\n' {
				result.push(i);
			}
		}
		// If there are bytes left add it to vector
		if *result.last().unwrap_or(&(usize::MAX - 1)) != bytes.len() - 1 {
			result.push(bytes.len() - 1);
		}

		let eof = string.as_bytes().last() == Some(&b'\n');

		(result, eof)
	}

	pub fn parse_file(file: &String) -> Self {
		let parse = LspPos::parse_string(&file);
		LspPos { ranges: parse.0, eof: parse.1 }
	}

	pub fn get_ranges(&self) -> &Vec<usize> { &self.ranges }

	pub fn get_eof(&self) -> bool { self.eof.clone() }

	// used to get a valid lsp position for the current file
	fn lsp_to_lsp(&self, lsp_pos: &lsp_types::Position) -> lsp_types::Position {
		self.byte_to_lsp(self.lsp_to_byte(lsp_pos))
	}

	pub fn line_char(&self, line: usize) -> usize {
		// a useful case to expect
		if line >= self.ranges.len() {
			return 0; // no characters exist on that line
		}

		let upper = self.ranges.get(line).unwrap_or(&0);
		// self.ranges[-1] = 0
		if line == 0 {
			return *upper + 1;
		}

		let lower = self.ranges.get(line - 1).unwrap_or(&0);
		upper - lower
	}

	pub fn lsp_to_byte(&self, lsp_pos: &lsp_types::Position) -> usize {
		// O(1) time complexity
		// file is empty
		if self.ranges.is_empty() {
			return 0;
		}

		// line greater than contain, return last byte + 1
		if lsp_pos.line as usize >= self.ranges.len() {
			return *self.ranges.last().unwrap() + 1;
		}

		// calculate upper byte and number of character in the line
		let upper = &self.ranges[lsp_pos.line as usize];
		let line_char = self.line_char(lsp_pos.line as usize);

		let mut char = lsp_pos.character as usize;
		// if inputed character is greater than line max character, set byte offset to max character
		if char >= line_char {
			char = line_char - 1;
		}

		// calculate byte offset from character
		upper - (line_char - 1 - char)
	}

	pub fn lsp_range_to_byte_range(&self, lsp_range: &lsp_types::Range) -> std::ops::Range<usize> {
		let start = self.lsp_to_byte(&lsp_range.start);
		let end = self.lsp_to_byte(&lsp_range.end);
		start..end
	}

	// O(log(n))
	pub fn byte_to_lsp(&self, byte_pos: usize) -> lsp_types::Position {
		// byte position greater than end of current file
		if byte_pos > *self.ranges.last().unwrap_or(&0) {
			return Position { line: self.ranges.len() as u32, character: 0 }; // return next position of last line
		}

		let mut low = 0;
		let mut high = self.ranges.len();
		// binary search
		while low < high {
			let mid = (low + high) / 2;
			if self.ranges[mid] == byte_pos {
				low = mid;
				break;
			} else if self.ranges[mid] < byte_pos {
				low = mid + 1;
			} else {
				high = mid;
			}
		}
		// calculate character position in byte offset
		let lower = if low == 0 { 0 } else { self.ranges[low - 1] + 1 };
		let char = byte_pos - lower;
		Position { line: low as u32, character: char as u32 }
	}

	pub fn byte_range_to_lsp_range(&self, byte_range: &std::ops::Range<usize>) -> lsp_types::Range {
		let start = self.byte_to_lsp(byte_range.start);
		let end = self.byte_to_lsp(byte_range.end);
		analyzer_abstractions::lsp_types::Range::new(start, end)
	}

	// used to update ranges from TextDocumentContentChangeEvent
	// will lazily add as only parse the text to be added
	// optimal for large files with small changes
	pub fn lazy_add(&mut self, changes: &TextDocumentContentChangeEvent) {
		// The whole file got changes || file was empty, so reparse as new file
		if changes.range.is_none() || self.ranges.is_empty() {
			*self = LspPos::parse_file(&changes.text);
			return;
		}

		// calculate position in current file
		let start_pos = self.lsp_to_lsp(&changes.range.unwrap().start); // inclusive
		let end_pos_exc = self.lsp_to_lsp(&changes.range.unwrap().end); // exclusive

		// undefined behaviour
		if start_pos > end_pos_exc {
			panic!(
				"range.start: {:?} is greater than range.end: {:?} in TextDocumentContentChangeEvent!",
				start_pos, end_pos_exc
			)
		}

		// parse input
		let (mut additional_ranges, eof) = LspPos::parse_string(&changes.text);
		let addition_byte: i64 = additional_ranges.last().map_or(-1, |value| *value as i64);

		// align additions to their placement in current file
		let start_byte = self.lsp_to_byte(&start_pos);
		let end_byte = self.lsp_to_byte(&end_pos_exc);
		for elm in &mut additional_ranges {
			*elm += start_byte;
		}

		// caching frequent conversions and calculation
		let mut start_line = start_pos.line as usize;
		let end_line = end_pos_exc.line as usize;
		let end_char = end_pos_exc.character as usize;
		let range_size = self.ranges.len();

		// need to make addition calculations for head and tail of new additions
		let tailing_end_char = self.line_char(end_line) - end_char;
		if additional_ranges.is_empty() {
			// special cases
			let end_line_byte = *self.ranges.get(end_line).unwrap_or(self.ranges.last().unwrap());
			let val = end_line_byte.wrapping_sub(end_byte).wrapping_add(start_byte) as i64;
			// we're deleteing the whole file
			if val < 0 {
				self.ranges.clear();
				self.eof = false;
				return;
			}

			// The case for deleting nothing to end of file
			if start_line == range_size {
				return;
			}

			// The change is just a deletion
			if tailing_end_char != 0 || start_pos.character != 0 {
				additional_ranges.push(val as usize);
			}
		} else {
			// \n is our line break, if adding to end of file don't make duplicate range
			if changes.text.as_bytes().last() == Some(&b'\n') && end_line != range_size {
				additional_ranges.push(*additional_ranges.last().unwrap());
			}
			*additional_ranges.last_mut().unwrap() += tailing_end_char;
		}

		// we're adding to end of file
		// if it doesn't has eof flag then merge addition onto end
		// if it does add a new index
		if start_line == range_size && !self.eof {
			start_line -= 1;
		}

		// update eof flag
		if end_line == range_size {
			self.eof = eof;
		}

		// remove old ranges and add new ranges
		let len = additional_ranges.len();
		let s = (start_line).min(range_size);
		let e = (end_line + 1).min(range_size);
		self.ranges.splice(s..e, additional_ranges);	// used for performance benifits

		// realign tail of old ranges
		let diff = (addition_byte + 1) - (end_byte as i64 - start_byte as i64);
		for elm in self.ranges.iter_mut().skip(start_line + len) {
			*elm = (*elm as i64 + diff) as usize;
		}
	}
}
