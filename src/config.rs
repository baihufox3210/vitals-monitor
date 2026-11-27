use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, io, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    Gpu,
    Vram,
    Cpu,
    Ram,
    GameProc,
    Disk,
    Network,
    Fps,
    FrameStats,
    Dlss,
    FrameTime,
}

impl Metric {
    pub const ALL: [Self; 11] = [
        Self::Gpu,
        Self::Vram,
        Self::Cpu,
        Self::Ram,
        Self::GameProc,
        Self::Disk,
        Self::Network,
        Self::Fps,
        Self::FrameStats,
        Self::Dlss,
        Self::FrameTime,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Gpu => "GPU",
            Self::Vram => "VRAM",
            Self::Cpu => "CPU",
            Self::Ram => "RAM",
            Self::GameProc => "GAME",
            Self::Disk => "DISK",
            Self::Network => "NET",
            Self::Fps => "FPS",
            Self::FrameStats => "STABILITY",
            Self::Dlss => "DLSS / FG",
            Self::FrameTime => "Frame time",
        }
    }
    pub fn class(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::Vram => "vram",
            Self::Cpu => "cpu",
            Self::Ram => "ram",
            Self::GameProc => "game",
            Self::Disk => "disk",
            Self::Network => "net",
            Self::Fps => "fps",
            Self::FrameStats => "frame-stats",
            Self::Dlss => "dlss",
            Self::FrameTime => "frame-time",
        }
    }

    pub fn field_defs(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Gpu => &[
                ("util", "使用率"),
                ("temp", "溫度"),
                ("clock", "核心時脈"),
                ("power", "功耗"),
                ("fan", "風扇轉速"),
            ],
            Self::Vram => &[("used", "已用 / 總容量"), ("clock", "記憶體時脈")],
            Self::Cpu => &[("usage", "使用率"), ("temp", "溫度"), ("clock", "最高時脈")],
            Self::Ram => &[("used", "已用 / 總容量"), ("swap", "Swap 用量")],
            Self::GameProc => &[
                ("cpu", "遊戲 CPU%"),
                ("ram", "遊戲記憶體"),
                ("threads", "執行緒數"),
                ("uptime", "運行時間"),
            ],
            Self::Disk => &[
                ("read", "讀取速率"),
                ("write", "寫入速率"),
                ("temp", "NVMe 溫度"),
            ],
            Self::Network => &[("rx", "下載速率"), ("tx", "上傳速率")],
            Self::Fps => &[("native", "原生 FPS"), ("output", "輸出 FPS")],
            Self::FrameStats => &[
                ("p50", "P50 中位數"),
                ("p95", "P95 幀率"),
                ("low", "低點 FPS"),
            ],
            Self::Dlss => &[
                ("sr", "SR 耗時"),
                ("fg", "FG 耗時與倍率"),
                ("res", "解析度縮放"),
            ],
            Self::FrameTime => &[
                ("avg", "平均影格時間"),
                ("graph", "30 秒波形圖"),
                ("p95", "P95 時間"),
                ("peak", "尖峰時間"),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub id: String,
    pub enabled: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Tile {
    pub metric: Metric,
    pub enabled: bool,
    #[serde(default)]
    pub fields: Vec<Field>,
}

impl Tile {
    pub fn default_for(metric: Metric, enabled: bool) -> Self {
        Self {
            metric,
            enabled,
            fields: metric
                .field_defs()
                .iter()
                .map(|&(id, _)| Field {
                    id: id.into(),
                    enabled: true,
                })
                .collect(),
        }
    }

    pub fn normalize_fields(&mut self) {
        let mut saved: HashMap<String, bool> =
            self.fields.drain(..).map(|f| (f.id, f.enabled)).collect();
        self.fields = self
            .metric
            .field_defs()
            .iter()
            .map(|&(id, _)| Field {
                id: id.into(),
                enabled: saved.remove(id).unwrap_or(true),
            })
            .collect();
    }

    pub fn wants_field(&self, id: &str) -> bool {
        self.enabled
            && self
                .fields
                .iter()
                .find(|f| f.id == id)
                .is_none_or(|f| f.enabled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlacklistEntry {
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub tiles: Vec<Tile>,
    pub columns: u32,
    pub width: i32,
    pub x: i32,
    pub y: i32,
    pub corner: u32,
    pub hover_hide: bool,
    pub overlay: bool,
    pub auto_show: bool,
    pub auto_detect: bool,
    pub css: String,
    pub target: String,
    pub timing_path: String,
    pub gnumon: String,
    #[serde(default)]
    pub blacklist: Vec<BlacklistEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tiles: Metric::ALL
                .into_iter()
                .map(|metric| Tile::default_for(metric, true))
                .collect(),
            columns: 1,
            width: 440,
            x: 16,
            y: 8,
            corner: 0,
            hover_hide: true,
            overlay: false,
            auto_show: true,
            auto_detect: false,
            css: String::new(),
            target: "Client-Win64-Shipping.exe".into(),
            timing_path: discover_timing(),
            gnumon: env::var("HOME")
                .map(|home| format!("{home}/gnumon/build/gnumon-cli"))
                .unwrap_or_default(),
            blacklist: Vec::new(),
        }
    }
}

fn discover_timing() -> String {
    let home = PathBuf::from(env::var_os("HOME").unwrap_or_default());
    let data = env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let steam = data.join("Steam");
    let libraries =
        fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")).unwrap_or_default();
    let roots = std::iter::once(steam).chain(libraries.lines().filter_map(|line| {
        let mut fields = line.split('"');
        fields.next()?;
        if fields.next()? != "path" {
            return None;
        }
        fields.next()?;
        Some(PathBuf::from(fields.next()?))
    }));
    let relative = "steamapps/common/Wuthering Waves/Wuthering Waves Game/Client/Binaries/Win64/dlssg_sm86/logs/frame_timing.txt";
    roots
        .map(|root| root.join(relative))
        .find(|file| file.parent().is_some_and(|p| p.is_dir()))
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn config_path() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("vitals-monitor/settings.json")
}

fn legacy_config_path() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("framehud/settings.json")
}

impl Config {
    pub fn load() -> io::Result<Self> {
        let path = config_path();
        if !path.exists() {
            let legacy = legacy_config_path();
            if legacy.exists() {
                if let Some(dir) = path.parent() {
                    fs::create_dir_all(dir)?;
                }
                // Keep the old file in place so downgrading still finds the settings.
                fs::copy(&legacy, &path)?;
            }
        }
        let raw = match fs::read(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        Self::parse(&raw)
    }

    pub fn parse(raw: &[u8]) -> io::Result<Self> {
        let mut config: Self = serde_json::from_slice(raw)?;
        let mut seen = std::collections::HashSet::new();
        config.tiles.retain(|tile| seen.insert(tile.metric));
        let frame_time_pos = config
            .tiles
            .iter()
            .position(|t| t.metric == Metric::FrameTime);
        let mut added = Metric::ALL
            .into_iter()
            .filter(|m| !seen.contains(m))
            .map(|metric| Tile::default_for(metric, true))
            .collect::<Vec<_>>();
        if let Some(pos) = frame_time_pos {
            for tile in added.drain(..) {
                let insert_at = config
                    .tiles
                    .iter()
                    .position(|t| t.metric == Metric::FrameTime)
                    .unwrap_or(pos);
                config.tiles.insert(insert_at, tile);
            }
        } else {
            config.tiles.extend(added);
        }
        for tile in &mut config.tiles {
            tile.normalize_fields();
        }
        config.columns = config.columns.clamp(1, 3);
        config.corner = config.corner.min(3);
        config.width = config.width.clamp(180, 1000);
        config.x = config.x.clamp(0, 500);
        config.y = config.y.clamp(0, 500);
        Ok(config)
    }

    pub fn save(&self) -> io::Result<()> {
        let path = config_path();
        fs::create_dir_all(path.parent().expect("configuration parent"))?;
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(&temp, bytes)?;
        fs::rename(&temp, &path).inspect_err(|_| {
            let _ = fs::remove_file(&temp);
        })
    }

    pub fn wants(&self, metric: Metric) -> bool {
        self.tiles
            .iter()
            .any(|tile| tile.metric == metric && tile.enabled)
    }

    pub fn wants_field(&self, metric: Metric, id: &str) -> bool {
        self.tiles
            .iter()
            .find(|tile| tile.metric == metric)
            .is_some_and(|tile| tile.wants_field(id))
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, Metric};

    #[test]
    fn normalizes_fields_on_legacy_config() {
        let legacy = br#"{"tiles":[{"metric":"gpu","enabled":false}]}"#;
        let cfg = Config::parse(legacy).unwrap();
        assert!(!cfg.wants(Metric::Gpu));
        assert!(!cfg.wants_field(Metric::Gpu, "util"));
        assert!(cfg.wants(Metric::Cpu));
        assert!(cfg.wants_field(Metric::Cpu, "temp"));
        let gpu = cfg.tiles.iter().find(|t| t.metric == Metric::Gpu).unwrap();
        assert_eq!(gpu.fields.len(), 5);
        assert!(gpu.fields.iter().all(|f| f.enabled));
    }
}
