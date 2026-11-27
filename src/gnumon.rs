use std::{
    fs,
    io::{ErrorKind, Read},
    os::fd::AsRawFd,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Gnumon {
    child: Option<Child>,
    pid: Option<u32>,
    buffer: Vec<u8>,
    column: Option<usize>,
    latest: Option<(Instant, f64)>,
    retry: Option<Instant>,
}

impl Gnumon {
    pub fn sample(&mut self, path: &Path, pid: Option<u32>) -> Option<f64> {
        if self.pid != pid
            || self
                .child
                .as_mut()
                .is_some_and(|c| c.try_wait().ok().flatten().is_some())
        {
            self.stop();
        }
        let pid = pid?;
        if !fs::metadata(format!("/dev/shm/gnumon_ring_{pid}")).is_ok_and(|m| m.is_file()) {
            if self.child.is_some() {
                self.stop();
            }
            return None;
        }
        if self.child.is_none() && self.retry.is_none_or(|t| t <= Instant::now()) {
            self.retry = Some(Instant::now() + Duration::from_secs(2));
            let mut child = Command::new(path)
                .args(["--pid", &pid.to_string(), "--output_stdout"])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .stdout(Stdio::piped())
                .spawn()
                .ok()?;
            let fd = child.stdout.as_ref()?.as_raw_fd();
            // ChildStdout owns this descriptor. Read without blocking the GTK event loop.
            if unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) } == -1 {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            self.pid = Some(pid);
            self.child = Some(child);
        }
        let output = self.child.as_mut()?.stdout.as_mut()?;
        let mut bytes = [0u8; 16384];
        match output.read(&mut bytes) {
            Ok(n) if n > 0 => self.buffer.extend_from_slice(&bytes[..n]),
            Err(e) if e.kind() != ErrorKind::WouldBlock => {
                self.stop();
                return None;
            }
            _ => (),
        }
        if let Some(end) = self.buffer.iter().rposition(|b| *b == b'\n') {
            for line in self.buffer[..end].split(|b| *b == b'\n') {
                let Ok(line) = std::str::from_utf8(line) else {
                    continue;
                };
                let columns = || line.trim_end().split(',');
                if let Some(index) = columns().position(|col| col.trim_matches('"') == "FPS") {
                    self.column = Some(index);
                    continue;
                }
                if let Some(fps) = self
                    .column
                    .and_then(|i| columns().nth(i))
                    .and_then(|col| col.trim_matches('"').parse::<f64>().ok())
                    .filter(|n| n.is_finite() && *n > 0.)
                {
                    self.latest = Some((Instant::now(), fps));
                }
            }
            self.buffer.drain(..=end);
        }
        if self.buffer.len() > 65536 {
            self.buffer.clear();
        }
        self.latest
            .filter(|(t, _)| t.elapsed() <= Duration::from_secs(3))
            .map(|(_, fps)| fps)
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.pid = None;
        self.latest = None;
        self.column = None;
        self.buffer.clear();
    }
}
impl Drop for Gnumon {
    fn drop(&mut self) {
        self.stop();
    }
}
