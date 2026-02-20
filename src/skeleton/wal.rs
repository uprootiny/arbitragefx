use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug)]
pub struct Wal {
    writer: BufWriter<File>,
}

impl Wal {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn append(&mut self, line: &str) {
        let _ = writeln!(self.writer, "{}", line);
        let _ = self.writer.flush();
    }
}
