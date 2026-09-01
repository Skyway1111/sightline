//! Line starts as CPython counts them: the one home of byte offset to
//! `(line, col)` (R1).

/// Byte offsets of the line starts. CPython translates `\r\n` and a lone
/// `\r` to `\n` before parsing, so all three end a line; `\x0c` and U+2028
/// do not.
pub struct Lines {
    starts: Vec<u32>,
}

impl Lines {
    pub fn new(source: &str) -> Lines {
        let bytes = source.as_bytes();
        let mut starts = vec![0u32];
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\n' => {
                    i += 1;
                    starts.push(i as u32);
                }
                b'\r' => {
                    i += if bytes.get(i + 1) == Some(&b'\n') {
                        2
                    } else {
                        1
                    };
                    starts.push(i as u32);
                }
                _ => i += 1,
            }
        }
        Lines { starts }
    }

    /// 1-based line and UTF-8 byte column, as CPython's `col_offset` counts.
    pub fn pos(&self, offset: u32) -> (u32, u32) {
        let line = self.starts.partition_point(|s| *s <= offset) - 1;
        (line as u32 + 1, offset - self.starts[line])
    }
}
