use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const DEFAULT_CHECKPOINT_STRIDE: usize = 1024;

#[derive(Debug, Clone)]
pub struct SparseFileSession {
    pub session_id: String,
    pub path: PathBuf,
    pub byte_length: u64,
    pub line_count: usize,
    pub checkpoint_stride: usize,
    pub checkpoints: Vec<(usize, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseViewportSlice {
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

#[derive(Debug, Default)]
pub struct SparseFileSessionManager {
    sessions: RwLock<HashMap<String, SparseFileSession>>,
}

impl SparseFileSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub fn open_session(&self, path: &Path) -> Result<SparseFileSession, String> {
        let path = path.to_path_buf();
        let session = build_sparse_session(path)?;
        self.sessions
            .write()
            .insert(session.session_id.clone(), session.clone());
        Ok(session)
    }

    pub fn get_session(&self, session_id: &str) -> Option<SparseFileSession> {
        self.sessions.read().get(session_id).cloned()
    }

    pub fn close_session(&self, session_id: &str) -> bool {
        self.sessions.write().remove(session_id).is_some()
    }

    pub fn read_viewport(
        &self,
        session_id: &str,
        start_line: usize,
        line_count: usize,
    ) -> Result<SparseViewportSlice, String> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| format!("unknown sparse session: {}", session_id))?;
        read_sparse_viewport(&session, start_line, line_count)
    }
}

fn build_sparse_session(path: PathBuf) -> Result<SparseFileSession, String> {
    let file = File::open(&path).map_err(|err| err.to_string())?;
    let metadata = file.metadata().map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(file);
    let mut checkpoints = vec![(0usize, 0u64)];
    let mut offset = 0u64;
    let mut current_line = 0usize;
    let mut buf = Vec::with_capacity(8192);

    loop {
        buf.clear();
        let bytes = reader.read_until(b'\n', &mut buf).map_err(|err| err.to_string())?;
        if bytes == 0 {
            break;
        }
        offset += bytes as u64;
        current_line += 1;
        if current_line % DEFAULT_CHECKPOINT_STRIDE == 0 {
            checkpoints.push((current_line, offset));
        }
    }

    let line_count = if metadata.len() == 0 {
        0
    } else if current_line == 0 {
        1
    } else {
        current_line
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1);
    Ok(SparseFileSession {
        session_id: format!("sparse_{}", COUNTER.fetch_add(1, Ordering::SeqCst)),
        path,
        byte_length: metadata.len(),
        line_count,
        checkpoint_stride: DEFAULT_CHECKPOINT_STRIDE,
        checkpoints,
    })
}

fn read_sparse_viewport(
    session: &SparseFileSession,
    start_line: usize,
    line_count: usize,
) -> Result<SparseViewportSlice, String> {
    if session.line_count == 0 || line_count == 0 || start_line >= session.line_count {
        return Ok(SparseViewportSlice {
            start_line,
            end_line: start_line,
            content: String::new(),
        });
    }

    let nearest = session
        .checkpoints
        .iter()
        .take_while(|(line, _)| *line <= start_line)
        .last()
        .copied()
        .unwrap_or((0, 0));
    let target_end = (start_line + line_count).min(session.line_count);

    let mut file = File::open(&session.path).map_err(|err| err.to_string())?;
    file.seek(SeekFrom::Start(nearest.1))
        .map_err(|err| err.to_string())?;
    let mut reader = BufReader::new(file);
    let mut current_line = nearest.0;
    let mut buf = Vec::with_capacity(8192);

    while current_line < start_line {
        buf.clear();
        let bytes = reader.read_until(b'\n', &mut buf).map_err(|err| err.to_string())?;
        if bytes == 0 {
            return Ok(SparseViewportSlice {
                start_line,
                end_line: current_line,
                content: String::new(),
            });
        }
        current_line += 1;
    }

    let mut content = Vec::new();
    while current_line < target_end {
        buf.clear();
        let bytes = reader.read_until(b'\n', &mut buf).map_err(|err| err.to_string())?;
        if bytes == 0 {
            break;
        }
        content.extend_from_slice(&buf);
        current_line += 1;
    }

    let content = String::from_utf8(content).map_err(|err| err.to_string())?;
    Ok(SparseViewportSlice {
        start_line,
        end_line: current_line,
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{}_{}", name, stamp))
    }

    #[test]
    fn sparse_session_builds_metadata_and_viewport() {
        let path = temp_path("monaco_sparse_viewport.txt");
        let content: String = (0..5000).map(|i| format!("line-{}\n", i)).collect();
        fs::write(&path, &content).unwrap();

        let manager = SparseFileSessionManager::new();
        let session = manager.open_session(&path).unwrap();
        assert_eq!(session.line_count, 5000);
        assert_eq!(session.checkpoint_stride, DEFAULT_CHECKPOINT_STRIDE);
        assert!(session.byte_length > 0);

        let slice = manager.read_viewport(&session.session_id, 2048, 4).unwrap();
        assert_eq!(slice.start_line, 2048);
        assert_eq!(slice.end_line, 2052);
        assert_eq!(
            slice.content,
            "line-2048\nline-2049\nline-2050\nline-2051\n"
        );

        fs::remove_file(path).ok();
    }

    #[test]
    fn sparse_viewport_handles_empty_file() {
        let path = temp_path("monaco_sparse_empty.txt");
        fs::write(&path, "").unwrap();

        let manager = SparseFileSessionManager::new();
        let session = manager.open_session(&path).unwrap();
        assert_eq!(session.line_count, 0);

        let slice = manager.read_viewport(&session.session_id, 0, 10).unwrap();
        assert_eq!(slice.content, "");
        assert_eq!(slice.end_line, 0);

        fs::remove_file(path).ok();
    }
}
