use lsp_types::{self, Position, TextDocumentContentChangeEvent};
use logos::Source; // for slice

#[derive(Clone, Debug)]
pub struct LspFile {
	file: String,       // File content, use OsString?
	ranges: Vec<usize>, // Each element represents the line, last byte position (see test_parse_file())
}

impl LspFile {
	pub fn new(file: &String) -> Self {
		let ranges = LspFile::parse_string(&file);
		LspFile { file: file.clone(), ranges }
	}

	pub fn get_file_content(&self) -> &String { &self.file }

	pub fn get_ranges(&self) -> &Vec<usize> { &self.ranges }

	// helper function
	fn parse_string(string: &String) -> Vec<usize> {
		let mut result: Vec<usize> = Vec::new();
		if string.is_empty() {
			return result;
		}

		let chars = string.chars();
		let mut byte_count = 0;
		for (_, c) in chars.enumerate() {
			byte_count += c.len_utf8();
			if c == '\n' {
				result.push(byte_count - 1);
			}
		}

		// If there are bytes left add it to vector
		if *result.last().unwrap_or(&(usize::MAX - 1)) != byte_count - 1 {
			result.push(byte_count - 1);
		}

		result
	}

	// used to get a valid lsp position for the current file
	fn lsp_to_lsp(&self, lsp_pos: &lsp_types::Position) -> lsp_types::Position {
		self.byte_to_lsp(self.lsp_to_byte(lsp_pos))
	}

	pub fn line_char(&self, line: usize) -> usize {
		// a useful case to expect
		if line >= self.ranges.len() {
			return 0; // no characters exist on that line
		}

		let start_pos = if line == 0 { 0 } else { self.ranges.get(line - 1).unwrap_or(&0) + 1 };

		let slice = self.file.slice(start_pos..self.ranges[line] + 1).unwrap_or("");
		slice.chars().count()
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

		let start_byte =
			if lsp_pos.line == 0 { 0 } else { self.ranges.get(lsp_pos.line as usize - 1).unwrap_or(&0) + 1 };

		// get byte offset for character position in line
		let slice = self.file.slice(start_byte..self.ranges[lsp_pos.line as usize]).unwrap_or("").chars();
		let mut byte_count = 0;
		for (i, c) in slice.enumerate() {
			if i == lsp_pos.character as usize {
				break;
			}
			byte_count += c.len_utf8();
		}

		start_byte + byte_count
	}

	pub fn lsp_range_to_byte_range(&self, lsp_range: &lsp_types::Range) -> std::ops::Range<usize> {
		let start = self.lsp_to_byte(&lsp_range.start);
		let end = self.lsp_to_byte(&lsp_range.end);
		start..end
	}

	// O(log(n))
	pub fn byte_to_lsp(&self, byte_pos: usize) -> lsp_types::Position {
		// byte position greater than end of current file
		if self.ranges.is_empty() {
			return Position { line: 0, character: 0 };
		}

		if byte_pos > *self.ranges.last().unwrap_or(&0) {
			return Position { line: self.ranges.len() as u32, character: 0 }; // return next position of last line
		}

		let line = self.ranges.binary_search(&byte_pos).unwrap_or_else(|x| x);

		// calculate character position in byte offset
		let mut byte_count = if line == 0 { 0 } else { self.ranges[line - 1] + 1 };
		let slice = self.file.slice(byte_count..self.ranges[line]).unwrap_or("").chars();
		let mut char = slice.clone().count();

		for (i, c) in slice.enumerate() {
			byte_count += c.len_utf8();
			if byte_count > byte_pos {
				char = i;
				break;
			}
		}

		Position { line: line as u32, character: char as u32 }
	}

	pub fn byte_range_to_lsp_range(&self, byte_range: &std::ops::Range<usize>) -> lsp_types::Range {
		let start = self.byte_to_lsp(byte_range.start);
		let end = self.byte_to_lsp(byte_range.end);
		lsp_types::Range::new(start, end)
	}

	// used to update ranges from TextDocumentContentChangeEvent
	// will lazily add as only parse the text to be added
	// optimal for large files with small changes
	pub fn lazy_add(&mut self, changes: &TextDocumentContentChangeEvent) {
		// The whole file got changes || file was empty, so reparse as new file
		if changes.range.is_none() || self.ranges.is_empty() {
			*self = LspFile::new(&changes.text);
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
		let mut additional_ranges = LspFile::parse_string(&changes.text);
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
		let range_size = self.ranges.len();

		// need to make addition calculations for head and tail of new additions
		let tailing_end_bytes = self.lsp_to_byte(&Position { line: end_line as u32 + 1, character: 0 }) - end_byte;

		// special cases if change text is empty
		if additional_ranges.is_empty() {
			let end_line_byte = *self.ranges.get(end_line).unwrap_or(self.ranges.last().unwrap());
			let val = end_line_byte.wrapping_sub(end_byte).wrapping_add(start_byte) as i64;
			// we're deleteing the whole file
			if val < 0 {
				self.file.clear();
				self.ranges.clear();
				return;
			}

			// The case for deleting nothing to end of file
			if start_line == range_size {
				return;
			}

			// The change is just a deletion
			if tailing_end_bytes != 0 || start_pos.character != 0 {
				additional_ranges.push(val as usize);
			}
		} else {
			// \n is our line break, if adding to end of file don't make duplicate range
			if changes.text.chars().last() == Some('\n') && end_line != range_size {
				additional_ranges.push(*additional_ranges.last().unwrap());
			}
			*additional_ranges.last_mut().unwrap() += tailing_end_bytes;
		}

		// we're adding to end of file
		// if it doesn't has eof flag then merge addition onto end
		// if it does add a new index
		if start_line == range_size && self.file.chars().last() != Some('\n') {
			start_line -= 1;
		}

		// update file
		let range = start_byte..end_byte;
		//info!("replacing range {:?} of {:?} with {:?}", range, &self.file[range.clone()], &changes.text);
		self.file.replace_range(range, &changes.text);

		// remove old ranges and add new ranges
		let len = additional_ranges.len();
		let s = (start_line).min(range_size);
		let e = (end_line + 1).min(range_size);
		self.ranges.splice(s..e, additional_ranges); // used for performance benefits

		// realignment of tail end of old ranges
		let diff = (addition_byte + 1) - (end_byte as i64 - start_byte as i64);
		for elm in self.ranges.iter_mut().skip(start_line + len) {
			*elm = (*elm as i64 + diff) as usize;
		}
	}
}
