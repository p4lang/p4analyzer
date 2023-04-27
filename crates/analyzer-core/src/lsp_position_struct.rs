use analyzer_abstractions::lsp_types::{self, TextDocumentContentChangeEvent, Position};

#[derive(Clone)]
pub struct LspPos {
    ranges: Vec<std::ops::Range<usize>>
}

impl LspPos {
    // helper function
    fn parse_string(string: &String) -> Vec<std::ops::Range<usize>> {
        let mut result: Vec<std::ops::Range<usize>> = Vec::new();
        let mut start = 0;
        let bytes = string.as_bytes();
    
        for i in 0..bytes.len() {
            if bytes[i] == b'\n' {
                result.push(start..i);
                start = i + 1;
            }
        }
        // If there are bytes left add it to vector
        if start < bytes.len() {
            result.push(start..bytes.len()-1);
        }

        result
    }

    pub fn parse_file(file: &String) -> Self {
        LspPos{ranges: LspPos::parse_string(&file)}
    }

    pub fn get_ranges(&self) -> Vec<std::ops::Range<usize>> {
        self.ranges.clone()
    }

    // used if a file has been changed to update the ranged list lazily
    pub fn lazy_add(&mut self, changes: &TextDocumentContentChangeEvent ) {
        // The whole file got changes, so reparse as new file
        if changes.range.is_none() {
            *self = LspPos::parse_file(&changes.text);
            return;
        }
        
        // aliases
        let start_pos = &changes.range.unwrap().start;
        let end_pos = &changes.range.unwrap().end;
        if start_pos > end_pos {
            panic!("range.start: {:?} is greater than range.end: {:?} in TextDocumentContentChangeEvent!", start_pos, end_pos)
        }
        // calculate new text as LSP
        let mut addition_lsp = LspPos::parse_string(&changes.text);
        if addition_lsp.is_empty() {
            addition_lsp.push(std::ops::Range{start: 0, end: 0})
        }
        if changes.text.ends_with("\n") {  // parse_string() doesn't create new row for trailing end_line but we do
            addition_lsp.push(std::ops::Range{start: addition_lsp.last().unwrap().end, end: addition_lsp.last().unwrap().end})
        }
        let addition = addition_lsp.last().unwrap().end;
        let start_byte = self.lsp_to_byte(start_pos);
        for elm in &mut addition_lsp {
            elm.start += start_byte;
            elm.end += start_byte;
        }
        addition_lsp.first_mut().unwrap().start = self.lsp_to_byte(&Position{ line: start_pos.line, character: 0 });    // In the splice self.ranges[start_pos.line] will be replaced
        
        // calculate size differece 
        let offset = if changes.text.is_empty() { 0 } else { 1 };   // Range.end is exclusive so need to do + 1, but also need to do -1 if text is empty
        let deletion = self.lsp_range_to_byte_range(&lsp_types::Range{ start: *start_pos, end: *end_pos });
        let diff: i64 = addition as i64 - (deletion.end as i64 - deletion.start as i64) + offset;

        addition_lsp.last_mut().unwrap().end = self.lsp_to_byte(&Position{ line: end_pos.line, character: u32::MAX }).wrapping_add(diff as usize); // In the splice self.ranges[end_pos.line] will be replaced

        // remove old ranges and add new
        let mut range = start_pos.line as usize..end_pos.line as usize + 1;
        if range.start > self.ranges.len() {    // We're editing end of file, so restrict it for splice
            addition_lsp[0].start += 1;         // 
            range.start = self.ranges.len();
        }
        if range.end > self.ranges.len() { range.end = self.ranges.len()}  // We're editing end of file, so restrict it for splice
        self.ranges.splice(range, addition_lsp);

        // readjustment to old ranges
        for range in self.ranges.iter_mut().skip(start_pos.line as usize + 1) {
            range.start = range.start.wrapping_add(diff as usize);
            range.end = range.end.wrapping_add(diff as usize);
        }
    }

    pub fn lsp_to_byte(&self, lsp_pos: &lsp_types::Position) -> usize {
        // O(1) time complexity
        // line greater than contain, return last byte
        if lsp_pos.line as usize >= self.ranges.len() {
            return self.ranges.last().unwrap().end;
        }

        // calculate byte offset from character
        let range = &self.ranges[lsp_pos.line as usize];
        let char_max_size = range.end - range.start;
        let char = lsp_pos.character as usize;
        // if inputed character is greater than line max character, set byte offset to max character
        let char = if char > char_max_size { char_max_size } else { char };
        range.start + char
    }

    pub fn lsp_range_to_byte_range(&self, lsp_range: &lsp_types::Range) -> std::ops::Range<usize> {
        let start = self.lsp_to_byte(&lsp_range.start);
        let end = self.lsp_to_byte(&lsp_range.end);
        start..end
    }

    pub fn byte_to_lsp(&self, byte_pos: usize) -> lsp_types::Position {
        // if position is greater than held, return last line, last character 
        if byte_pos > self.ranges.last().unwrap().end {
            let line: usize = self.ranges.len() - 1;
            let character: u32 = (self.ranges[line].end - self.ranges[line].start).try_into().unwrap();
            return lsp_types::Position{line: line.try_into().unwrap(), character};
        }
        
        let mut low = 0;
        let mut high = self.ranges.len() - 1;
        // ranged binary search O(log(n))
        while low <= high {
            let mid = (low + high) / 2;

            if byte_pos < self.ranges[mid].start {
                high = mid - 1;
            } else if byte_pos > self.ranges[mid].end {
                low = mid + 1;
            } else {
                let line = mid as u32;
                let character = (byte_pos - self.ranges[mid].start) as u32;
                return lsp_types::Position{line, character};
            }
        }
        // only reachable if there is a bug in code (construction of LspPos::ranges or this method)
        unreachable!();
    }

    pub fn byte_range_to_lsp_range(&self, byte_range: &std::ops::Range<usize>) -> lsp_types::Range {
        let start = self.byte_to_lsp(byte_range.start);
        let end = self.byte_to_lsp( byte_range.end);
        analyzer_abstractions::lsp_types::Range::new(start, end)
    }
}
