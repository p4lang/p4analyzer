use analyzer_abstractions::lsp_types::{self, TextDocumentContentChangeEvent};

#[derive(Clone, Debug)]
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

    // used to update ranges from TextDocumentContentChangeEvent
    // will lazily add as only parse the text to be added
    pub fn lazy_add(&mut self, changes: &TextDocumentContentChangeEvent) {
        // The whole file got changes || file was empty, so reparse as new file
        if changes.range.is_none() || self.ranges.is_empty() {
            *self = LspPos::parse_file(&changes.text);
            return;
        }

        // calculate position in current file  
        let start_pos = self.lsp_to_lsp(&changes.range.unwrap().start);     // inclusive
        let end_pos_exc = self.lsp_to_lsp(&changes.range.unwrap().end);      // exclusive
        let end_pos_inc = self.byte_to_lsp(self.lsp_to_byte(&changes.range.unwrap().end) - 1);   // inclusive
        // undefined behaviour
        if start_pos > end_pos_exc {
            panic!("range.start: {:?} is greater than range.end: {:?} in TextDocumentContentChangeEvent!", start_pos, end_pos_exc)
        }

        // calculate new text as LSP
        let mut addition_lsp = LspPos::parse_string(&changes.text);

        // calculate stats
        let start_line = start_pos.line as usize;
        let removed_lines = (end_pos_exc.line - start_pos.line) as usize + 1;
        let add_lines = addition_lsp.len();
        let size_diff = self.lsp_to_byte(&end_pos_inc) as i32 - self.lsp_to_byte(&start_pos) as i32
        - addition_lsp.last().map_or(0, |last| last.end as i32);
        
        let byte_offet = self.ranges[start_line].start + start_pos.character as usize;
        let end_chars = self.ranges[end_pos_inc.line as usize].end - self.ranges[end_pos_inc.line as usize].start - end_pos_inc.character as usize;

        // realign addition ranges
        for elm in addition_lsp.iter_mut() {
            elm.start += byte_offet;
            elm.end += byte_offet;
        }

        // Head and Tail needs additional realignment with exist ranges
        if !addition_lsp.is_empty() {
            addition_lsp.first_mut().unwrap().start = self.ranges[start_line].start;
            addition_lsp.last_mut().unwrap().end += end_chars;
        }

        // removes changes ranges
        for _ in 0..removed_lines {
            self.ranges.remove(start_line);
        }

        // add new lines (does the vector backward for inplace insertion)
        for elm in addition_lsp.iter_mut().rev() {
            self.ranges.insert(start_line, elm.clone());
        }

        // realign values
        for elm in self.ranges.iter_mut().skip(start_line + add_lines - removed_lines + 1) {
            elm.start = (elm.start as i32 - size_diff) as usize;
            elm.end = (elm.end as i32 - size_diff) as usize;
        }
    }

    // used to get a valid lsp position for the current file
    fn lsp_to_lsp(&self, lsp_pos: &lsp_types::Position) -> lsp_types::Position {
        self.byte_to_lsp(self.lsp_to_byte(lsp_pos))
    }

    pub fn lsp_to_byte(&self, lsp_pos: &lsp_types::Position) -> usize {
        // O(1) time complexity
        // file is empty
        if self.ranges.is_empty() {     
            return 0;
        }

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
        // file is empty
        if self.ranges.is_empty() {
            return lsp_types::Position{line: 0, character: 0};
        }

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
