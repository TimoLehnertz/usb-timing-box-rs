use core::fmt;

use crate::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassingBatch {
    pub requested_start: u32,
    pub passings: Vec<Passing>,
}

impl PassingBatch {
    pub fn next_start_index(&self) -> u32 {
        self.requested_start + self.passings.len() as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passing {
    pub transponder_id: String,
    pub fields: Vec<String>,
}

impl fmt::Display for Passing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_line())
    }
}

impl Passing {
    pub fn from_line(line: &str) -> Result<Self, Error> {
        let mut parts = line.split(';');
        let transponder_id =
            parts.next().ok_or_else(|| Error::Protocol("passing line has no transponder id".to_string()))?.to_string();

        let fields = parts.map(ToString::to_string).collect();
        Ok(Self { transponder_id, fields })
    }

    pub fn as_line(&self) -> String {
        let mut line = self.transponder_id.clone();
        for field in &self.fields {
            line.push(';');
            line.push_str(field);
        }
        line
    }

    /// Returns the internal decoder timestamp field (if present and parseable).
    ///
    /// According to the protocol examples this is the 2nd field after transponder id.
    pub fn timestamp_ticks(&self) -> Option<u64> {
        self.fields.get(1).and_then(|v| u64::from_str_radix(v.trim(), 16).ok())
    }
}
