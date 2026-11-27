use std::{
    collections::VecDeque,
    fmt::Write as _,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::Path,
    time::{Duration, SystemTime},
};

const TAIL: u64 = 16 * 1024;
const MAX_READ: u64 = 128 * 1024;

#[derive(Default, Clone, Copy, Debug)]
pub struct Frames {
    pub render: f64,
    pub generated: Option<f64>,
    pub average_ms: f64,
    pub peak_ms: f64,
    pub batch_peak_ms: Option<f64>,
    pub p50_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub low_1pct_fps: Option<f64>,
    pub sr_ms: Option<f64>,
    pub fg_ms: Option<f64>,
    pub fg_factor: Option<u8>,
    pub sr_res: Option<(u32, u32)>,
    pub out_res: Option<(u32, u32)>,
}

#[derive(Clone, Copy, Debug)]
struct FrameSample {
    t: f64,
    dt: f64,
    generated: u8,
    sr_ms: f32,
    fg_ms: f32,
}

#[derive(Clone, Copy, Debug)]
struct SecondSummary {
    render: f64,
    generated: Option<f64>,
    average_ms: f64,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    peak_ms: f64,
    sr_ms: Option<f64>,
    fg_ms: Option<f64>,
    fg_factor: Option<u8>,
}

#[derive(Default)]
pub struct FrameTail {
    file: Option<File>,
    id: (u64, u64),
    offset: u64,
    partial: Vec<u8>,
    discard: bool,
    samples: VecDeque<FrameSample>,
    saw_frame_row: bool,
    summary: Option<SecondSummary>,
    sr_res: Option<(u32, u32)>,
    out_res: Option<(u32, u32)>,
    bytes: Vec<u8>,
    scratch: Vec<f64>,
}

fn parse_res(token: &str) -> Option<(u32, u32)> {
    let (w, h) = token.split_once('x')?;
    let w = w.parse::<u32>().ok().filter(|&v| v > 0)?;
    let h = h.parse::<u32>().ok().filter(|&v| v > 0)?;
    Some((w, h))
}

type Resolution = Option<(u32, u32)>;

fn parse_context(line: &str) -> Option<(Resolution, Resolution)> {
    if !line.starts_with("# context ") {
        return None;
    }
    let mut cols = line.split_ascii_whitespace();
    let mut sr_res = None;
    let mut out_res = None;
    while let Some(tok) = cols.next() {
        if tok == "sr" {
            sr_res = cols.next().and_then(parse_res);
            if cols.next() == Some("->") {
                out_res = cols.next().and_then(parse_res);
            }
            break;
        }
    }
    Some((sr_res, out_res))
}

fn parse_second(line: &str) -> Option<SecondSummary> {
    let mut cols = line.split_ascii_whitespace();
    if cols.next()? != "#" || cols.next()? != "second" {
        return None;
    }
    cols.next()?.parse::<u64>().ok()?;

    let mut fps = None;
    let mut dt_avg = None;
    let mut dt_p50 = None;
    let mut dt_p95 = None;
    let mut dt_max = None;
    let mut sr_avg = None;
    let mut fg_avg = None;
    let mut generated_factor = None;

    let parse_f = |tok: Option<&str>| {
        tok.and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.)
    };

    while let Some(key) = cols.next() {
        match key {
            "fps" => fps = parse_f(cols.next()),
            "dt" => {
                dt_avg = parse_f(cols.next());
                dt_p50 = parse_f(cols.next());
                dt_p95 = parse_f(cols.next());
                dt_max = parse_f(cols.next());
            }
            "sr" => {
                sr_avg = parse_f(cols.next());
                let _ = (cols.next(), cols.next(), cols.next());
            }
            "fg" => {
                fg_avg = parse_f(cols.next());
                let _ = (cols.next(), cols.next(), cols.next());
            }
            "n" => {
                generated_factor = cols
                    .next()
                    .and_then(|v| v.parse::<u8>().ok())
                    .filter(|n| (1..=5).contains(n));
            }
            _ => {}
        }
    }

    let render = fps.or_else(|| dt_avg.map(|avg| 1000. / avg))?;
    let average_ms = dt_avg.unwrap_or_else(|| 1000. / render);
    let peak_ms = dt_max.unwrap_or(average_ms).max(average_ms);
    let generated = generated_factor.map(|n| render * (1. + f64::from(n)));
    Some(SecondSummary {
        render,
        generated,
        average_ms,
        p50_ms: dt_p50,
        p95_ms: dt_p95,
        peak_ms,
        sr_ms: sr_avg,
        fg_ms: fg_avg,
        fg_factor: generated_factor.map(|n| n + 1),
    })
}

impl FrameTail {
    pub fn sample(&mut self, path: &Path) -> Option<Frames> {
        let meta = fs::metadata(path).ok()?;
        let id = (meta.dev(), meta.ino());
        if self.file.is_none()
            || self.id != id
            || meta.len() < self.offset
            || meta.len().saturating_sub(self.offset) > MAX_READ
        {
            let mut file = File::open(path).ok()?;
            self.offset = meta.len().saturating_sub(TAIL);
            file.seek(SeekFrom::Start(self.offset)).ok()?;
            self.discard = self.offset > 0;
            self.samples.clear();
            self.saw_frame_row = false;
            self.summary = None;
            self.sr_res = None;
            self.out_res = None;
            self.partial.clear();
            self.id = id;
            self.file = Some(file);
        }
        if SystemTime::now()
            .duration_since(meta.modified().ok()?)
            .unwrap_or_default()
            > Duration::from_secs(3)
        {
            self.samples.clear();
            self.saw_frame_row = false;
            self.summary = None;
            return None;
        }
        self.bytes.clear();
        self.bytes.append(&mut self.partial);
        let count = self
            .file
            .as_mut()?
            .take(MAX_READ)
            .read_to_end(&mut self.bytes)
            .ok()?;
        self.offset += count as u64;
        let mut batch_peak: Option<f64> = None;
        let mut summary_peak: Option<f64> = None;
        let mut lines = self.bytes.split_inclusive(|b| *b == b'\n');
        for line in &mut lines {
            if !line.ends_with(b"\n") {
                self.partial.extend_from_slice(line);
                break;
            }
            if std::mem::take(&mut self.discard) {
                continue;
            }
            let Ok(line) = std::str::from_utf8(line) else {
                continue;
            };
            if line.contains("==== session") {
                self.samples.clear();
                self.saw_frame_row = false;
                self.summary = None;
                self.sr_res = None;
                self.out_res = None;
                batch_peak = None;
                summary_peak = None;
                continue;
            }
            if line.starts_with("# cap:") {
                self.samples.clear();
                self.saw_frame_row = false;
                batch_peak = None;
                continue;
            }
            if let Some((sr_res, out_res)) = parse_context(line) {
                if sr_res.is_some() {
                    self.sr_res = sr_res;
                }
                if out_res.is_some() {
                    self.out_res = out_res;
                }
                continue;
            }
            if let Some(summary) = parse_second(line) {
                if !self.saw_frame_row {
                    self.samples.clear();
                    batch_peak = None;
                }
                self.saw_frame_row = false;
                self.summary = Some(summary);
                summary_peak = Some(summary.peak_ms);
                continue;
            }
            let mut cols = line.split_ascii_whitespace();
            if cols.next().and_then(|n| n.parse::<u64>().ok()).is_none() {
                continue;
            }
            let Some(t) = cols
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite())
            else {
                continue;
            };
            let Some(dt) = cols
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.)
            else {
                continue;
            };
            let sr_ms = cols
                .next()
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite() && *v > 0.)
                .unwrap_or(0.);
            let fg_ms = cols
                .next()
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite() && *v > 0.)
                .unwrap_or(0.);
            let generated = cols
                .next()
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|n| *n <= 5)
                .unwrap_or(0);
            if self.samples.back().is_some_and(|s| s.t > t) {
                self.samples.clear();
                batch_peak = None;
            }
            self.saw_frame_row = true;
            self.samples.push_back(FrameSample {
                t,
                dt,
                generated,
                sr_ms,
                fg_ms,
            });
            while self.samples.len() > 4096 || self.samples.front().is_some_and(|s| s.t < t - 1000.)
            {
                self.samples.pop_front();
            }
            batch_peak = Some(batch_peak.map_or(dt, |p| p.max(dt)));
        }
        if self.partial.len() > TAIL as usize {
            self.partial.clear();
            self.discard = true;
        }
        if self.bytes.capacity() > (TAIL as usize) * 2 {
            self.bytes.clear();
            self.bytes.shrink_to(TAIL as usize);
        }

        if !self.samples.is_empty() {
            let mut sum = 0.0_f64;
            let mut peak = 0.0_f64;
            let mut gen_sum = 0_u32;
            let mut max_n = 0_u8;
            let mut sr_sum = 0.0_f64;
            let mut sr_cnt = 0_u32;
            let mut fg_sum = 0.0_f64;
            let mut fg_cnt = 0_u32;
            self.scratch.clear();
            for s in &self.samples {
                sum += s.dt;
                peak = peak.max(s.dt);
                gen_sum += u32::from(s.generated);
                max_n = max_n.max(s.generated);
                if s.sr_ms > 0. {
                    sr_sum += f64::from(s.sr_ms);
                    sr_cnt += 1;
                }
                if s.fg_ms > 0. {
                    fg_sum += f64::from(s.fg_ms);
                    fg_cnt += 1;
                }
                self.scratch.push(s.dt);
            }
            if sum > 0. {
                let count = self.samples.len() as f64;
                self.scratch.sort_by(f64::total_cmp);
                let len = self.scratch.len();
                let p50_ms = Some(self.scratch[len / 2]);
                let p95_ms = Some(self.scratch[(len * 95 / 100).min(len - 1)]);
                let p99_ms = self.scratch[(len * 99 / 100).min(len - 1)];
                return Some(Frames {
                    render: 1000. * count / sum,
                    generated: (gen_sum > 0).then(|| 1000. * (count + f64::from(gen_sum)) / sum),
                    average_ms: sum / count,
                    peak_ms: peak,
                    batch_peak_ms: batch_peak,
                    p50_ms,
                    p95_ms,
                    low_1pct_fps: (p99_ms > 0.).then(|| 1000. / p99_ms),
                    sr_ms: (sr_cnt > 0).then(|| sr_sum / f64::from(sr_cnt)),
                    fg_ms: (fg_cnt > 0).then(|| fg_sum / f64::from(fg_cnt)),
                    fg_factor: (max_n > 0).then(|| max_n + 1),
                    sr_res: self.sr_res,
                    out_res: self.out_res,
                });
            }
        }

        self.summary.map(|summary| Frames {
            render: summary.render,
            generated: summary.generated,
            average_ms: summary.average_ms,
            peak_ms: summary.peak_ms,
            batch_peak_ms: summary_peak,
            p50_ms: summary.p50_ms,
            p95_ms: summary.p95_ms,
            low_1pct_fps: (summary.peak_ms > 0.).then(|| 1000. / summary.peak_ms),
            sr_ms: summary.sr_ms,
            fg_ms: summary.fg_ms,
            fg_factor: summary.fg_factor,
            sr_res: self.sr_res,
            out_res: self.out_res,
        })
    }
}

#[derive(Default)]
pub struct GameProbe {
    pid: Option<u32>,
    path: String,
    buf: Vec<u8>,
    exe_name: Vec<u8>,
}

impl GameProbe {
    fn matches_pid(&mut self, pid: u32, target: &[u8]) -> bool {
        self.path.clear();
        let _ = write!(&mut self.path, "/proc/{pid}/cmdline");
        let Ok(file) = File::open(&self.path) else {
            return false;
        };
        self.buf.clear();
        if file.take(4096).read_to_end(&mut self.buf).is_err() {
            return false;
        }
        let first = self.buf.split(|b| *b == 0).next().unwrap_or_default();
        first
            .rsplit(|b| *b == b'/' || *b == b'\\')
            .next()
            .unwrap_or_default()
            .eq_ignore_ascii_case(target)
    }

    /// Extracts the executable name from the process at `pid`.
    fn exe_of(&mut self, pid: u32) -> Option<()> {
        self.path.clear();
        let _ = write!(&mut self.path, "/proc/{pid}/cmdline");
        let file = File::open(&self.path).ok()?;
        self.buf.clear();
        file.take(4096).read_to_end(&mut self.buf).ok()?;
        let first = self.buf.split(|b| *b == 0).next().unwrap_or_default();
        let name = first
            .rsplit(|b| *b == b'/' || *b == b'\\')
            .next()
            .unwrap_or_default();
        if name.is_empty() {
            return None;
        }
        self.exe_name.clear();
        self.exe_name.extend_from_slice(name);
        Some(())
    }

    fn is_blacklisted(&self, blacklist: &[&str]) -> bool {
        let name = std::str::from_utf8(&self.exe_name).unwrap_or("");
        blacklist.iter().any(|b| name.eq_ignore_ascii_case(b))
    }

    pub fn find(&mut self, target: &str) -> Option<u32> {
        let target_bytes = target.as_bytes();
        if let Some(pid) = self.pid
            && self.matches_pid(pid, target_bytes)
        {
            return Some(pid);
        }
        let mut found = None;
        for entry in fs::read_dir("/proc").ok()?.flatten() {
            let name = entry.file_name();
            let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            if self.matches_pid(pid, target_bytes) {
                found = Some(pid);
                break;
            }
        }
        self.pid = found;
        found
    }

    /// Find any running game process, excluding blacklisted names.
    /// Returns the PID of the first candidate.
    pub fn find_any(&mut self, blacklist: &[&str]) -> Option<u32> {
        if let Some(pid) = self.pid {
            // Check if the cached PID still runs and isn't blacklisted.
            if self.exe_of(pid).is_some() && !self.is_blacklisted(blacklist) {
                // Still alive and not blacklisted — keep it.
                return Some(pid);
            } else {
                self.pid = None;
            }
        }
        let mut found = None;
        for entry in fs::read_dir("/proc").ok()?.flatten() {
            let name = entry.file_name();
            let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            if self.exe_of(pid).is_none() {
                continue;
            }
            // Skip kernel threads and common system processes.
            let exe_str = std::str::from_utf8(&self.exe_name).unwrap_or("");
            if exe_str.is_empty() || self.is_blacklisted(blacklist) {
                continue;
            }
            // Skip known non-game processes.
            let system_processes = [
                "systemd",
                "bash",
                "sh",
                "zsh",
                "fish",
                "Xorg",
                "Xwayland",
                "wayland",
                "sway",
                "hyprland",
                "gamescope",
                "pipewire",
                "pulseaudio",
                "wireplumber",
                "dbus-daemon",
                "polkitd",
                "upowerd",
                "vitals-monitor",
                "framehud",
                "dms",
                "quickshell",
            ];
            let is_system = system_processes
                .iter()
                .any(|s| exe_str.eq_ignore_ascii_case(s));
            if is_system {
                continue;
            }
            found = Some(pid);
            break;
        }
        self.pid = found;
        found
    }
}

#[cfg(test)]
mod tests {
    use super::FrameTail;
    use std::{
        fs::{self, OpenOptions},
        io::Write,
    };

    #[test]
    fn live_tail_ignores_partial_lines_and_drops_old_session() {
        let dir = std::env::temp_dir().join(format!("vitals-monitor-tail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        let path = dir.join("timing");
        fs::write(&path, "1 16 16 - - 3\n2 32 16 - - 3").unwrap();
        let mut tail = FrameTail::default();
        let first = tail.sample(&path).unwrap();
        assert_eq!(first.render, 62.5);
        assert_eq!(first.generated, Some(250.));
        let mut writer = OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(writer).unwrap();
        let next = tail.sample(&path).unwrap();
        assert_eq!(next.batch_peak_ms, Some(16.));
        writeln!(writer, "# ==== session b\n1 10 10 - - -").unwrap();
        let after = tail.sample(&path).unwrap();
        assert_eq!(after.render, 100.);
        assert_eq!(after.generated, None);
        let replacement = dir.join("replacement");
        fs::write(&replacement, "1 8 8 - - 1\n").unwrap();
        fs::rename(replacement, &path).unwrap();
        assert_eq!(tail.sample(&path).unwrap().generated, Some(250.));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn falls_back_to_second_summaries_after_frame_row_cap() {
        let dir =
            std::env::temp_dir().join(format!("vitals-monitor-second-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        let path = dir.join("timing");
        fs::write(
            &path,
            "# context from frame 0: sr 1506x847 -> 2560x1440 preset ? kernels game\n1 16 16 - - -\n# cap: frame rows stop at frame 2\n# second 1  frames 60  fps 60.00  dt 16.667 16.600 18.000 22.500  sr 0.900 0.900 1.000 1.100  fg 7.000 7.000 7.500 8.000  n 3  gdt 16.667 16.600 18.000 22.500  gpu 98%  fb 55%\n",
        )
        .unwrap();

        let mut tail = FrameTail::default();
        let capped = tail.sample(&path).unwrap();
        assert_eq!(capped.render, 60.0);
        assert_eq!(capped.generated, Some(240.0));
        assert_eq!(capped.average_ms, 16.667);
        assert_eq!(capped.peak_ms, 22.5);
        assert_eq!(capped.batch_peak_ms, Some(22.5));
        assert_eq!(capped.p50_ms, Some(16.6));
        assert_eq!(capped.p95_ms, Some(18.0));
        assert_eq!(capped.sr_ms, Some(0.9));
        assert_eq!(capped.fg_ms, Some(7.0));
        assert_eq!(capped.fg_factor, Some(4));
        assert_eq!(capped.sr_res, Some((1506, 847)));
        assert_eq!(capped.out_res, Some((2560, 1440)));

        let idle = tail.sample(&path).unwrap();
        assert_eq!(idle.render, 60.0);
        assert_eq!(idle.batch_peak_ms, None);

        let mut writer = OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(
            writer,
            "# second 2  frames 50  fps 50.00  dt 20.000 19.800 21.000 24.000  sr 0.800 0.800 0.900 1.000  gdt 20.000 19.800 21.000 24.000  gpu 90%  fb 50%"
        )
        .unwrap();
        let next = tail.sample(&path).unwrap();
        assert_eq!(next.render, 50.0);
        assert_eq!(next.generated, None);
        assert_eq!(next.average_ms, 20.0);
        assert_eq!(next.peak_ms, 24.0);
        assert_eq!(next.batch_peak_ms, Some(24.0));
        assert_eq!(next.fg_ms, None);
        assert_eq!(next.fg_factor, None);

        fs::remove_dir_all(dir).unwrap();
    }
}
