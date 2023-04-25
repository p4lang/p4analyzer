use analyzer_abstractions::lsp_types::{self};

pub struct LspPos {
    ranges: Vec<std::ops::Range<usize>>
}


impl LspPos {
    pub fn parse_file(file: &String) -> Self {
        let mut result: Vec<std::ops::Range<usize>> = Vec::new();
        let mut start = 0;
        let bytes = file.as_bytes();
    
        for i in 0..bytes.len() {
            if bytes[i] == b'\n' {
                result.push(start..i+1);
                start = i + 1;
            }
        }
        // If there are bytes left add it to vector
        if start < bytes.len() {
            result.push(start..bytes.len());
        }
    
        LspPos{ranges: result}
    }

    // used if a file has been changed to update the ranged list lazily
    pub fn lazy_add(&mut self, input: String, pos: lsp_types::Position ) {

    }

    pub fn lsp_to_byte(&self, lsp_pos: lsp_types::Position) -> usize {
        // O(1)
        // panics if out of bound
        self.ranges[lsp_pos.line as usize].start + lsp_pos.character as usize
    }

    pub fn lsp_range_to_byte_range(&self, lsp_range: lsp_types::Range) -> std::ops::Range<usize> {
        let start = self.lsp_to_byte(lsp_range.start);
        let end = self.lsp_to_byte(lsp_range.end);
        start..end
    }

    pub fn byte_to_lsp(&self, byte_pos: usize) -> lsp_types::Position {
        // If position is greater than held, return last line, last character 
        if byte_pos > self.ranges.last().unwrap().end {
            let line = self.ranges.len();
            let character: u32 = (self.ranges[line - 1].end - self.ranges[line - 1].start).try_into().unwrap();
            return lsp_types::Position{line: line.try_into().unwrap(), character};
        }
        
        let mut low = 0;
        let mut high = self.ranges.len() - 1;
        // Binary range search O(log(n))
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
        // Only reachable if there is a bug in code (construction of LspPos::ranges or this method)
        unreachable!();
    }

    pub fn byte_range_to_lsp_range(&self, byte_range: std::ops::Range<usize>) -> lsp_types::Range {
        let start = self.byte_to_lsp(byte_range.start);
        let end = self.byte_to_lsp( byte_range.end);
        analyzer_abstractions::lsp_types::Range::new(start, end)
    }
}
