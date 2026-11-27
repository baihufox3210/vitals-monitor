use crate::config::{Config, Metric};
use libloading::Library;
use std::{
    ffi::c_void,
    fmt::Write as _,
    fs::{self, File},
    io::{self, BufRead, BufReader, ErrorKind, Read, Write},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

type Device = *mut c_void;

#[repr(C)]
#[derive(Default)]
struct Util {
    gpu: u32,
    memory: u32,
}

#[repr(C)]
#[derive(Default)]
struct Memory {
    total: u64,
    free: u64,
    used: u64,
}

struct Nvml {
    _library: Library,
    device: Device,
    shutdown: unsafe extern "C" fn() -> i32,
    utilization: unsafe extern "C" fn(Device, *mut Util) -> i32,
    memory: unsafe extern "C" fn(Device, *mut Memory) -> i32,
    temp: unsafe extern "C" fn(Device, u32, *mut u32) -> i32,
    clock: unsafe extern "C" fn(Device, u32, *mut u32) -> i32,
    power: unsafe extern "C" fn(Device, *mut u32) -> i32,
    fan: Option<unsafe extern "C" fn(Device, *mut u32) -> i32>,
}

impl Nvml {
    fn open() -> Option<Self> {
        unsafe {
            let lib = Library::new("libnvidia-ml.so.1").ok()?;
            let init = *lib
                .get::<unsafe extern "C" fn() -> i32>(b"nvmlInit_v2")
                .ok()?;
            let shutdown = *lib
                .get::<unsafe extern "C" fn() -> i32>(b"nvmlShutdown")
                .ok()?;
            let handle = *lib
                .get::<unsafe extern "C" fn(u32, *mut Device) -> i32>(
                    b"nvmlDeviceGetHandleByIndex_v2",
                )
                .ok()?;
            let utilization = *lib.get(b"nvmlDeviceGetUtilizationRates").ok()?;
            let memory = *lib.get(b"nvmlDeviceGetMemoryInfo").ok()?;
            let temp = *lib.get(b"nvmlDeviceGetTemperature").ok()?;
            let clock = *lib.get(b"nvmlDeviceGetClockInfo").ok()?;
            let power = *lib.get(b"nvmlDeviceGetPowerUsage").ok()?;
            let fan = lib
                .get::<unsafe extern "C" fn(Device, *mut u32) -> i32>(b"nvmlDeviceGetFanSpeed")
                .ok()
                .map(|s| *s);
            if init() != 0 {
                return None;
            }
            let mut device = std::ptr::null_mut();
            if handle(0, &mut device) != 0 {
                shutdown();
                return None;
            }
            Some(Self {
                _library: lib,
                device,
                shutdown,
                utilization,
                memory,
                temp,
                clock,
                power,
                fan,
            })
        }
    }

    fn write_snapshot(&self, out: &mut impl Write) -> io::Result<()> {
        let (mut util, mut temp, mut sm_clk, mut mem_clk, mut power, mut fan) =
            (Util::default(), 0, 0, 0, 0, 0);
        let mut mem = Memory::default();
        unsafe {
            let util_ok = (self.utilization)(self.device, &mut util) == 0;
            let temp_ok = (self.temp)(self.device, 0, &mut temp) == 0;
            let sm_ok = (self.clock)(self.device, 0, &mut sm_clk) == 0;
            let mem_clk_ok = (self.clock)(self.device, 2, &mut mem_clk) == 0;
            let power_ok = (self.power)(self.device, &mut power) == 0;
            let fan_ok = self.fan.is_some_and(|f| f(self.device, &mut fan) == 0);
            let mem_ok = (self.memory)(self.device, &mut mem) == 0;

            let mut field = |ok: bool, v: u64, sep: &str| -> io::Result<()> {
                if ok {
                    write!(out, "{v}{sep}")
                } else {
                    write!(out, "-{sep}")
                }
            };
            field(util_ok, u64::from(util.gpu), " ")?;
            field(util_ok, u64::from(util.memory), " ")?;
            field(temp_ok, u64::from(temp), " ")?;
            field(sm_ok, u64::from(sm_clk), " ")?;
            field(mem_clk_ok, u64::from(mem_clk), " ")?;
            field(power_ok, u64::from(power), " ")?;
            field(fan_ok, u64::from(fan), " ")?;
            field(mem_ok, mem.used, " ")?;
            field(mem_ok, mem.total, "\n")?;
        }
        out.flush()
    }
}

impl Drop for Nvml {
    fn drop(&mut self) {
        unsafe {
            (self.shutdown)();
        }
    }
}

pub fn nvml_worker() -> i32 {
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
    }
    let Some(nvml) = Nvml::open() else {
        return 1;
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = io::BufWriter::new(stdout.lock());
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if nvml.write_snapshot(&mut writer).is_err() {
                    break;
                }
            }
        }
    }
    0
}

fn push_part(out: &mut String, part: impl FnOnce(&mut String)) {
    if !out.is_empty() {
        out.push_str("  ");
    }
    part(out);
}

#[derive(Clone, Copy, Default)]
struct GpuSnapshot {
    util: Option<u64>,
    temp: Option<u64>,
    sm_clock: Option<u64>,
    mem_clock: Option<u64>,
    power_mw: Option<u64>,
    fan: Option<u64>,
    vram_used: Option<u64>,
    vram_total: Option<u64>,
}

impl GpuSnapshot {
    fn parse(line: &str) -> Option<Self> {
        let mut cols = line.split_ascii_whitespace();
        let opt = |tok: Option<&str>| tok.and_then(|v| (v != "-").then(|| v.parse().ok())?);
        let util = opt(cols.next());
        let _mem_util = opt(cols.next());
        let temp = opt(cols.next());
        let sm_clock = opt(cols.next());
        let mem_clock = opt(cols.next());
        let power_mw = opt(cols.next());
        let fan = opt(cols.next());
        let vram_used = opt(cols.next());
        let vram_total = opt(cols.next());
        Some(Self {
            util,
            temp,
            sm_clock,
            mem_clock,
            power_mw,
            fan,
            vram_used,
            vram_total,
        })
    }

    fn format_gpu(self, config: &Config) -> Option<String> {
        let mut out = String::new();
        if config.wants_field(Metric::Gpu, "util") {
            let u = self.util?;
            push_part(&mut out, |s| {
                let _ = write!(s, "{u}%");
            });
        }
        if config.wants_field(Metric::Gpu, "temp") {
            push_part(&mut out, |s| match self.temp {
                Some(t) => {
                    let _ = write!(s, "{t}°C");
                }
                None => s.push('—'),
            });
        }
        if config.wants_field(Metric::Gpu, "clock") {
            push_part(&mut out, |s| match self.sm_clock {
                Some(c) => {
                    let _ = write!(s, "{c}MHz");
                }
                None => s.push('—'),
            });
        }
        if config.wants_field(Metric::Gpu, "power") {
            push_part(&mut out, |s| match self.power_mw {
                Some(p) => {
                    let _ = write!(s, "{}W", (p + 500) / 1000);
                }
                None => s.push('—'),
            });
        }
        if config.wants_field(Metric::Gpu, "fan")
            && let Some(f) = self.fan
        {
            push_part(&mut out, |s| {
                let _ = write!(s, "F{f}%");
            });
        }
        (!out.is_empty()).then_some(out)
    }

    fn format_vram(self, config: &Config) -> Option<String> {
        let mut out = String::new();
        if config.wants_field(Metric::Vram, "used") {
            let (used, total) = (self.vram_used?, self.vram_total?);
            push_part(&mut out, |s| {
                let _ = write!(
                    s,
                    "{:.1} / {:.1} GiB",
                    used as f64 / 1073741824.,
                    total as f64 / 1073741824.
                );
            });
        }
        if config.wants_field(Metric::Vram, "clock")
            && let Some(clk) = self.mem_clock
        {
            push_part(&mut out, |s| {
                let _ = write!(s, "{clk}MHz");
            });
        }
        (!out.is_empty()).then_some(out)
    }
}

#[derive(Default)]
struct NvmlClient {
    child: Option<Child>,
    buf: Vec<u8>,
    latest: Option<(Instant, GpuSnapshot)>,
    retry: Option<Instant>,
}

impl NvmlClient {
    fn sample(&mut self, active: bool) -> Option<GpuSnapshot> {
        if !active {
            self.stop();
            return None;
        }
        if self
            .child
            .as_mut()
            .is_some_and(|c| c.try_wait().ok().flatten().is_some())
        {
            self.stop();
        }
        if self.child.is_none() && self.retry.is_none_or(|t| Instant::now() >= t) {
            self.retry = Some(Instant::now() + Duration::from_secs(15));
            if let Ok(exe) = std::env::current_exe()
                && let Ok(mut child) = Command::new(exe)
                    .arg("--nvml")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
            {
                if let Some(fd) = child.stdout.as_ref().map(AsRawFd::as_raw_fd)
                    && unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) } == -1
                {
                    let _ = child.kill();
                    let _ = child.wait();
                } else {
                    self.child = Some(child);
                }
            }
        }
        let child = self.child.as_mut()?;
        if let Some(stdin) = child.stdin.as_mut()
            && stdin.write_all(b"\n").is_err()
        {
            self.stop();
            return None;
        }
        if let Some(stdout) = child.stdout.as_mut() {
            let mut chunk = [0u8; 1024];
            loop {
                match stdout.read(&mut chunk) {
                    Ok(0) => {
                        self.stop();
                        return None;
                    }
                    Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(_) => {
                        self.stop();
                        return None;
                    }
                }
                if self.buf.len() > 8192 {
                    self.buf.clear();
                    break;
                }
            }
        }
        if let Some(end) = self.buf.iter().rposition(|b| *b == b'\n') {
            for line in self.buf[..end].split(|b| *b == b'\n') {
                if let Ok(text) = std::str::from_utf8(line)
                    && let Some(snap) = GpuSnapshot::parse(text)
                {
                    self.latest = Some((Instant::now(), snap));
                }
            }
            self.buf.drain(..=end);
        }
        self.latest
            .filter(|(t, _)| t.elapsed() <= Duration::from_secs(5))
            .map(|(_, snap)| snap)
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take());
            let _ = child.kill();
            let _ = child.wait();
        }
        self.buf.clear();
        self.latest = None;
    }
}

impl Drop for NvmlClient {
    fn drop(&mut self) {
        self.stop();
    }
}

fn read_into(path: impl AsRef<Path>, buf: &mut String) -> Option<()> {
    buf.clear();
    let mut file = File::open(path).ok()?;
    file.read_to_string(buf).ok()?;
    Some(())
}

fn numeric(path: impl AsRef<Path>, buf: &mut String) -> Option<u64> {
    read_into(path, buf)?;
    buf.trim().parse().ok()
}

fn cpu_ticks(buf: &mut String) -> Option<(u64, u64)> {
    buf.clear();
    let file = File::open("/proc/stat").ok()?;
    BufReader::new(file).read_line(buf).ok()?;
    let mut values = buf
        .split_ascii_whitespace()
        .skip(1)
        .take(8)
        .map(str::parse::<u64>);
    let (mut total, mut idle) = (0, 0);
    for i in 0..8 {
        let v = values.next()?.ok()?;
        total += v;
        if i == 3 || i == 4 {
            idle += v;
        }
    }
    Some((total, idle))
}

fn ram(buf: &mut String, show_used: bool, show_swap: bool) -> Option<String> {
    if !show_used && !show_swap {
        return None;
    }
    read_into("/proc/meminfo", buf)?;
    let (mut total, mut available, mut swap_total, mut swap_free) = (None, None, None, None);
    for line in buf.lines() {
        let mut fields = line.split_ascii_whitespace();
        match fields.next() {
            Some("MemTotal:") => total = fields.next().and_then(|v| v.parse::<u64>().ok()),
            Some("MemAvailable:") => available = fields.next().and_then(|v| v.parse::<u64>().ok()),
            Some("SwapTotal:") => swap_total = fields.next().and_then(|v| v.parse::<u64>().ok()),
            Some("SwapFree:") => swap_free = fields.next().and_then(|v| v.parse::<u64>().ok()),
            _ => {}
        }
        if total.is_some() && available.is_some() && swap_total.is_some() && swap_free.is_some() {
            break;
        }
    }
    let mut out = String::new();
    if show_used {
        let (total, available) = (total?, available?);
        push_part(&mut out, |s| {
            let _ = write!(
                s,
                "{:.1} / {:.1} GiB",
                total.saturating_sub(available) as f64 / 1048576.,
                total as f64 / 1048576.
            );
        });
    }
    if show_swap && let (Some(st), Some(sf)) = (swap_total, swap_free) {
        let swap_used_gib = st.saturating_sub(sf) as f64 / 1048576.;
        if !show_used {
            let swap_total_gib = st as f64 / 1048576.;
            push_part(&mut out, |s| {
                let _ = write!(s, "SW {swap_used_gib:.1} / {swap_total_gib:.1} GiB");
            });
        } else if swap_used_gib >= 0.1 {
            push_part(&mut out, |s| {
                let _ = write!(s, "+SW {swap_used_gib:.1}G");
            });
        }
    }
    (!out.is_empty()).then_some(out)
}

fn cpu_sensor() -> Option<PathBuf> {
    fs::read_dir("/sys/class/hwmon")
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find_map(|path| {
            let name = fs::read_to_string(path.join("name")).ok()?;
            matches!(name.trim(), "k10temp" | "coretemp" | "zenpower")
                .then(|| path.join("temp1_input"))
        })
}

fn nvme_sensors() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir("/sys/class/hwmon") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let name = fs::read_to_string(path.join("name")).ok()?;
            (name.trim() == "nvme").then(|| path.join("temp1_input"))
        })
        .collect()
}

fn cpufreq_paths() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let s = name.to_str()?;
            let rest = s.strip_prefix("cpu")?;
            if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let p = e.path().join("cpufreq/scaling_cur_freq");
            p.exists().then_some(p)
        })
        .collect()
}

fn physical_nics() -> Vec<String> {
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            p.join("device")
                .exists()
                .then(|| e.file_name().to_string_lossy().into_owned())
        })
        .collect()
}

fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1048576. {
        format!("{:.1}M/s", bytes_per_sec / 1048576.)
    } else if bytes_per_sec >= 1024. {
        format!("{:.0}K/s", bytes_per_sec / 1024.)
    } else {
        format!("{:.0}B/s", bytes_per_sec)
    }
}

fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[derive(Default)]
pub struct System {
    nvml: NvmlClient,
    ticks: Option<(u64, u64)>,
    sensor: Option<Option<PathBuf>>,
    nvmes: Option<Vec<PathBuf>>,
    cpufreqs: Option<Vec<PathBuf>>,
    nics: Option<Vec<String>>,
    proc_prev: Option<(u32, u64, u64)>,
    io_prev: Option<(u32, Instant, u64, u64)>,
    net_prev: Option<(Instant, u64, u64)>,
    buf: String,
    path_buf: String,
}

impl System {
    pub fn release(&mut self) {
        self.nvml.stop();
        self.ticks = None;
        self.proc_prev = None;
        self.io_prev = None;
        self.net_prev = None;
        if self.buf.capacity() > 16384 {
            self.buf = String::new();
        }
    }

    pub fn sample(
        &mut self,
        config: &Config,
        pid: Option<u32>,
        mut emit: impl FnMut(Metric, String),
    ) {
        let wants_gpu = config.wants(Metric::Gpu);
        let wants_vram = config.wants(Metric::Vram);
        let snap = self.nvml.sample(wants_gpu || wants_vram);
        if wants_gpu {
            emit(
                Metric::Gpu,
                snap.and_then(|s| s.format_gpu(config))
                    .unwrap_or_else(|| "--".into()),
            );
        }
        if wants_vram {
            emit(
                Metric::Vram,
                snap.and_then(|s| s.format_vram(config))
                    .unwrap_or_else(|| "--".into()),
            );
        }
        if config.wants(Metric::Ram) {
            let show_used = config.wants_field(Metric::Ram, "used");
            let show_swap = config.wants_field(Metric::Ram, "swap");
            emit(
                Metric::Ram,
                ram(&mut self.buf, show_used, show_swap).unwrap_or_else(|| "--".into()),
            );
        }

        let cpu_usage = config.wants_field(Metric::Cpu, "usage");
        let cpu_temp = config.wants_field(Metric::Cpu, "temp");
        let cpu_clock = config.wants_field(Metric::Cpu, "clock");
        let proc_cpu = config.wants_field(Metric::GameProc, "cpu");
        let sys_ticks = if cpu_usage || proc_cpu {
            cpu_ticks(&mut self.buf)
        } else {
            None
        };

        if config.wants(Metric::Cpu) {
            let mut out = String::new();
            if cpu_usage {
                let usage = sys_ticks
                    .zip(self.ticks)
                    .and_then(|((total, idle), (prev_total, prev_idle))| {
                        let elapsed = total.checked_sub(prev_total)?;
                        (elapsed > 0).then(|| {
                            format!(
                                "{:.0}%",
                                100. * elapsed.saturating_sub(idle.saturating_sub(prev_idle))
                                    as f64
                                    / elapsed as f64
                            )
                        })
                    })
                    .unwrap_or_else(|| "--%".into());
                self.ticks = sys_ticks;
                push_part(&mut out, |s| s.push_str(&usage));
            } else {
                self.ticks = None;
            }
            if cpu_temp {
                let sensor_path = self.sensor.get_or_insert_with(cpu_sensor).clone();
                let temp = sensor_path
                    .as_deref()
                    .and_then(|p| numeric(p, &mut self.buf))
                    .map_or("--°C".into(), |v| format!("{}°C", (v + 500) / 1000));
                push_part(&mut out, |s| s.push_str(&temp));
            }
            if cpu_clock {
                let freqs = self.cpufreqs.get_or_insert_with(cpufreq_paths);
                let mut max_khz = 0_u64;
                for i in 0..freqs.len() {
                    if let Some(khz) = numeric(&freqs[i], &mut self.buf) {
                        max_khz = max_khz.max(khz);
                    }
                }
                let clock = if max_khz > 0 {
                    format!("{}MHz", max_khz / 1000)
                } else {
                    "--MHz".into()
                };
                push_part(&mut out, |s| s.push_str(&clock));
            }
            emit(Metric::Cpu, if out.is_empty() { "--".into() } else { out });
        } else {
            self.ticks = None;
        }

        if config.wants(Metric::GameProc) {
            let val = pid
                .and_then(|pid| self.sample_proc(config, pid, sys_ticks))
                .unwrap_or_else(|| "--".into());
            emit(Metric::GameProc, val);
        } else {
            self.proc_prev = None;
        }

        if config.wants(Metric::Disk) {
            emit(Metric::Disk, self.sample_disk(config, pid));
        } else {
            self.io_prev = None;
        }

        if config.wants(Metric::Network) {
            emit(
                Metric::Network,
                self.sample_network(config).unwrap_or_else(|| "--".into()),
            );
        } else {
            self.net_prev = None;
        }
    }

    fn sample_proc(
        &mut self,
        config: &Config,
        pid: u32,
        sys_ticks: Option<(u64, u64)>,
    ) -> Option<String> {
        let want_cpu = config.wants_field(Metric::GameProc, "cpu");
        let want_ram = config.wants_field(Metric::GameProc, "ram");
        let want_threads = config.wants_field(Metric::GameProc, "threads");
        let want_uptime = config.wants_field(Metric::GameProc, "uptime");
        if !want_cpu && !want_ram && !want_threads && !want_uptime {
            return None;
        }
        self.path_buf.clear();
        let _ = write!(&mut self.path_buf, "/proc/{pid}/stat");
        read_into(&self.path_buf, &mut self.buf)?;
        let rest = self.buf.rsplit_once(')')?.1;
        let mut cols = rest.split_ascii_whitespace();
        let _state = cols.next()?;
        let utime = cols.nth(10)?.parse::<u64>().ok()?;
        let stime = cols.next()?.parse::<u64>().ok()?;
        let threads = cols.nth(4)?.parse::<u64>().ok()?;
        let starttime = cols.nth(1)?.parse::<u64>().ok()?;
        let rss_pages = cols.nth(1)?.parse::<u64>().ok()?;

        let mut out = String::new();
        if want_cpu {
            let proc_ticks = utime.saturating_add(stime);
            let cpu_str = match (self.proc_prev, sys_ticks) {
                (Some((prev_pid, prev_p, prev_s)), Some((cur_s, _))) if prev_pid == pid => {
                    let ds = cur_s.saturating_sub(prev_s);
                    if ds > 0 {
                        format!(
                            "{:.0}%",
                            100. * proc_ticks.saturating_sub(prev_p) as f64 / ds as f64
                        )
                    } else {
                        "--%".into()
                    }
                }
                _ => "--%".into(),
            };
            if let Some((cur_s, _)) = sys_ticks {
                self.proc_prev = Some((pid, proc_ticks, cur_s));
            }
            push_part(&mut out, |s| s.push_str(&cpu_str));
        } else {
            self.proc_prev = None;
        }
        if want_ram {
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as u64;
            let rss_gib = (rss_pages * page_size) as f64 / 1073741824.;
            push_part(&mut out, |s| {
                let _ = write!(s, "{rss_gib:.1} GiB");
            });
        }
        if want_threads {
            push_part(&mut out, |s| {
                let _ = write!(s, "{threads}T");
            });
        }
        if want_uptime
            && read_into("/proc/uptime", &mut self.buf).is_some()
            && let Some(sys_up) = self
                .buf
                .split_ascii_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
        {
            let clk_tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
            let proc_up = (sys_up as u64).saturating_sub(starttime / clk_tck);
            push_part(&mut out, |s| s.push_str(&format_uptime(proc_up)));
        }
        (!out.is_empty()).then_some(out)
    }

    fn sample_disk(&mut self, config: &Config, pid: Option<u32>) -> String {
        let want_read = config.wants_field(Metric::Disk, "read");
        let want_write = config.wants_field(Metric::Disk, "write");
        let want_temp = config.wants_field(Metric::Disk, "temp");
        let mut out = String::new();

        if (want_read || want_write)
            && let Some(pid) = pid
        {
            self.path_buf.clear();
            let _ = write!(&mut self.path_buf, "/proc/{pid}/io");
            if read_into(&self.path_buf, &mut self.buf).is_some() {
                let (mut r_bytes, mut w_bytes) = (None, None);
                for line in self.buf.lines() {
                    let mut cols = line.split_ascii_whitespace();
                    match cols.next() {
                        Some("read_bytes:") => {
                            r_bytes = cols.next().and_then(|v| v.parse::<u64>().ok())
                        }
                        Some("write_bytes:") => {
                            w_bytes = cols.next().and_then(|v| v.parse::<u64>().ok())
                        }
                        _ => {}
                    }
                }
                if let (Some(rb), Some(wb)) = (r_bytes, w_bytes) {
                    let now = Instant::now();
                    if let Some((prev_pid, prev_t, prev_r, prev_w)) = self.io_prev
                        && prev_pid == pid
                    {
                        let dt = now.duration_since(prev_t).as_secs_f64();
                        if dt > 0.1 {
                            if want_read {
                                let rr = rb.saturating_sub(prev_r) as f64 / dt;
                                push_part(&mut out, |s| {
                                    let _ = write!(s, "R {}", format_rate(rr));
                                });
                            }
                            if want_write {
                                let wr = wb.saturating_sub(prev_w) as f64 / dt;
                                push_part(&mut out, |s| {
                                    let _ = write!(s, "W {}", format_rate(wr));
                                });
                            }
                        }
                    }
                    self.io_prev = Some((pid, now, rb, wb));
                }
            }
        } else {
            self.io_prev = None;
        }

        if want_temp {
            let nvmes = self.nvmes.get_or_insert_with(nvme_sensors);
            let mut max_temp = 0_u64;
            for i in 0..nvmes.len() {
                if let Some(t) = numeric(&nvmes[i], &mut self.buf) {
                    max_temp = max_temp.max(t);
                }
            }
            if max_temp > 0 {
                push_part(&mut out, |s| {
                    let _ = write!(s, "{}°C", (max_temp + 500) / 1000);
                });
            }
        }

        if out.is_empty() { "--".into() } else { out }
    }

    fn sample_network(&mut self, config: &Config) -> Option<String> {
        let want_rx = config.wants_field(Metric::Network, "rx");
        let want_tx = config.wants_field(Metric::Network, "tx");
        if !want_rx && !want_tx {
            self.net_prev = None;
            return None;
        }
        let nics = self.nics.get_or_insert_with(physical_nics);
        read_into("/proc/net/dev", &mut self.buf)?;
        let (mut rx_total, mut tx_total) = (0_u64, 0_u64);
        for line in self.buf.lines().skip(2) {
            let Some((iface, rest)) = line.split_once(':') else {
                continue;
            };
            let iface = iface.trim();
            if nics.is_empty() {
                if iface == "lo" {
                    continue;
                }
            } else if !nics.iter().any(|n| n == iface) {
                continue;
            }
            let mut cols = rest.split_ascii_whitespace();
            let Some(rx) = cols.next().and_then(|v| v.parse::<u64>().ok()) else {
                continue;
            };
            let Some(tx) = cols.nth(7).and_then(|v| v.parse::<u64>().ok()) else {
                continue;
            };
            rx_total = rx_total.saturating_add(rx);
            tx_total = tx_total.saturating_add(tx);
        }

        let now = Instant::now();
        let out = self.net_prev.and_then(|(prev_t, prev_rx, prev_tx)| {
            let dt = now.duration_since(prev_t).as_secs_f64();
            (dt > 0.1).then(|| {
                let mut s = String::new();
                if want_rx {
                    let rx_rate = rx_total.saturating_sub(prev_rx) as f64 / dt;
                    push_part(&mut s, |out| {
                        let _ = write!(out, "↓ {}", format_rate(rx_rate));
                    });
                }
                if want_tx {
                    let tx_rate = tx_total.saturating_sub(prev_tx) as f64 / dt;
                    push_part(&mut s, |out| {
                        let _ = write!(out, "↑ {}", format_rate(tx_rate));
                    });
                }
                s
            })
        });
        self.net_prev = Some((now, rx_total, tx_total));
        out.filter(|s| !s.is_empty())
    }
}
