# Vitals Monitor

Linux Wayland 遊戲監控面板，使用 Rust、GTK 3 與 layer-shell。預設從 Steam 遊戲庫尋找《鳴潮》的 NGX 影格日誌，也可指定其他目標程序與來源。

「Vitals」取自主命徵象：把 FPS、影格時間、GPU 與 CPU 狀態當成遊戲的即時體檢數據。

## DMS 整合

DMS 2.1 以上可透過 Dash 的 **Vitals** 分頁調整指標、位置、資料來源、CSS 和程序黑名單。用每列右側的開關顯示或隱藏指標；細項在每次開啟面板時預設收合；點選已開啟的指標主列即可展開勾選，再點一次收合。把手和開關仍各自操作。拖曳左側六點把手可調整顯示順序，鍵盤也可在把手上按 **↑ / ↓** 排序。固定在頁首右側的總開關是草稿的一部分；下方 **預覽** 顯示草稿但不儲存，**套用** 才會儲存設定並更新 HUD。若套用後只想在遊戲運行時顯示，請在「位置」保留對應選項。

```sh
cargo build --release
install -Dm755 target/release/vitals-monitor ~/.local/bin/vitals-monitor
install -Dm644 dms/vitals-monitor-overlay.service ~/.config/systemd/user/vitals-monitor-overlay.service
mkdir -p ~/.config/dms/plugins/Vitals
cp dms/Vitals/*.qml dms/Vitals/plugin.json ~/.config/dms/plugins/Vitals/
rm -f ~/.config/dms/plugins/Vitals/FrameHudTab.qml
systemctl --user daemon-reload
systemctl --user enable vitals-monitor-overlay.service
dms ipc call plugin-scan rescan vitalsMonitor
dms ipc call plugins reload vitalsMonitor
dms ipc call plugins enable vitalsMonitor
dms ipc call dash open plugin_vitalsMonitor
```

從舊版 FrameHUD 升級時，另外刪除舊的服務與外掛：

```sh
systemctl --user disable --now framehud-overlay.service
rm -f ~/.config/systemd/user/framehud-overlay.service ~/.local/bin/framehud
rm -rf ~/.config/dms/plugins/FrameHUD
systemctl --user daemon-reload
```

遊戲內圖層及採集由 Rust 服務負責。DMS 只在載入設定或操作按鈕時啟動短暫的 CLI 請求。更新外掛時也請更新 `~/.local/bin/vitals-monitor`；舊版執行檔可載入頁面，但可能不會套用自動偵測及黑名單。更新執行檔後，以 `systemctl --user restart vitals-monitor-overlay.service` 重新啟動已在運行的服務。GTK 設定視窗也可以用 `cargo run --release` 獨立啟動，切換前先停止 DMS 的 HUD 服務，避免重複顯示。

建置需 Rust、GTK 3 和 gtk-layer-shell；HUD 需使用 niri 等支援 layer-shell 的 Wayland compositor。NVIDIA 指標使用系統 NVML。

設定儲存於 `$XDG_CONFIG_HOME/vitals-monitor/settings.json`，預設為 `~/.config/vitals-monitor/settings.json`。首次啟動時若只有舊版 `~/.config/framehud/settings.json`，會自動複製一份，舊檔保留不動。

## Vitals 與來源

FPS 採用 `原生 / 輸出` 格式。左側取最近一秒 NGX 真實影格時間，右側優先使用已有 GNUMON capture，其次以 NGX 日誌記錄的實際生成影格數估算。無資料顯示 `--`。Frame time 波形含 30 秒的平均與尖峰，時間採自 NGX Evaluate 的影格間隔。

GPU 提供使用率、溫度、時脈與功耗，另有 VRAM、CPU 使用率／溫度／時脈與 RAM。若某來源不支援對應指標會顯示缺值。

## 架構

- `config` 處理指標定義、排版與設定儲存。
- `frames` 增量讀取 NGX 日誌並追蹤程序，處理檔案輪替、截斷、半行與過期資料。
- `gnumon` 只在目標程序有 capture ring 時啟動一個 CLI，並採非阻塞讀取。
- `system` 重用 NVML 連線，每秒讀取所需的系統指標，CPU 溫度路徑會快取。
- `overlay` 管理 layer-shell、圖表與輸入區域。HUD 隱藏後停止高頻取樣。
- `settings` 提供版面、CSS 和來源設定。

以 `cargo run --release -- --sample /path/to/frame_timing.txt` 可直接讀取近期 NGX 資料。與舊版 HUD 並存時，兩套都會各自採集資料，日常使用建議只啟用一套。
