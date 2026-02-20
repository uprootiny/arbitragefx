use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Logger {
    run_id: String,
    writer: BufWriter<File>,
}

impl Logger {
    pub fn new(base: &Path, run_id: String) -> std::io::Result<Self> {
        let mut dir = PathBuf::from(base);
        dir.push(&run_id);
        create_dir_all(&dir)?;
        let file = File::create(dir.join("events.jsonl"))?;
        Ok(Self {
            run_id,
            writer: BufWriter::new(file),
        })
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn log(&mut self, event: &str, data: &str) {
        let _ = writeln!(self.writer, "{{\"event\":\"{}\",\"data\":{}}}", event, data);
        let _ = self.writer.flush();
    }
}
