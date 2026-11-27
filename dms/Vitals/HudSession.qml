pragma ComponentBehavior: Bound

import QtQuick
import Quickshell
import Quickshell.Io

Item {
    id: session
    visible: false
    required property bool live
    signal targetRequired
    readonly property string executable: Quickshell.env("HOME") + "/.local/bin/vitals-monitor"
    property var draft: null
    property string operation: ""
    property bool ready: false
    property bool preview: false
    property bool dirty: false
    property bool failed: false
    property string message: ""
    readonly property bool busy: operation !== ""
    readonly property ListModel blacklist: ListModel {}

    function patch(key, value) {
        if (!draft || draft[key] === value)
            return;
        draft = Object.assign({}, draft, {
            [key]: value
        });
        dirty = true;
    }

    function updateMetric(index, field, checked) {
        const tiles = draft.tiles.slice();
        const tile = Object.assign({}, tiles[index]);
        if (field < 0) {
            tile.enabled = checked;
        } else {
            tile.fields = tile.fields.slice();
            tile.fields[field] = Object.assign({}, tile.fields[field], {
                enabled: checked
            });
        }
        tiles[index] = tile;
        patch("tiles", tiles);
    }

    function moveMetric(from, to) {
        const tiles = draft.tiles.slice();
        if (from < 0 || from >= tiles.length || to < 0 || to >= tiles.length || from === to)
            return;
        tiles.splice(to, 0, tiles.splice(from, 1)[0]);
        patch("tiles", tiles);
    }

    function snapshot() {
        const entries = [];
        for (let i = 0; i < blacklist.count; i++) {
            const entry = blacklist.get(i);
            entries.push({
                name: entry.name,
                enabled: entry.active
            });
        }
        return Object.assign({}, draft, {
            blacklist: entries
        });
    }

    function run(op) {
        if (busy || (op !== "read" && !draft) || (op === "read" && draft))
            return;
        if ((op === "apply" || op === "preview") && !draft.target.trim()) {
            targetRequired();
            failed = true;
            message = "請輸入遊戲執行檔名稱。";
            return;
        }
        failed = false;
        message = "";
        operation = op;
        // env also gives us an exit code and stderr when the installed binary is missing.
        const args = ["/usr/bin/env", executable];
        if (op === "read")
            args.push("--read-config");
        else if (op === "stop-preview")
            args.push("--control", op);
        else
            args.push("--control", op, JSON.stringify(snapshot()));
        request.command = args;
        request.running = true;
    }

    function finish(code, status) {
        const op = operation;
        failed = code !== 0 || status !== 0;
        if (failed) {
            message = code === 127 ? "找不到 Vitals Monitor，請確認已安裝至 ~/.local/bin/vitals-monitor。" : requestError.text.trim() || "Vitals Monitor 指令執行失敗。";
        } else if (op === "read") {
            try {
                const data = JSON.parse(requestOutput.text);
                if (!data || !Array.isArray(data.tiles) || typeof data.target !== "string" || (data.blacklist !== undefined && !Array.isArray(data.blacklist)))
                    throw new Error("設定格式不相容，請更新 Vitals Monitor 執行檔。");
                blacklist.clear();
                for (const entry of data.blacklist ?? [])
                    blacklist.append({
                        name: entry.name,
                        active: entry.enabled
                    });
                draft = data;
            } catch (error) {
                failed = true;
                message = "無法讀取設定：" + error.message;
            }
        } else {
            preview = op === "preview";
            if (op === "apply")
                dirty = false;
            message = op === "apply" ? "設定已套用。" : op === "preview" ? "預覽中，尚未儲存。" : "預覽已關閉。";
        }
        operation = "";
        if (!live && preview && op !== "stop-preview")
            Qt.callLater(() => {
                if (!session.live && session.preview)
                    session.run("stop-preview");
            });
    }

    onLiveChanged: {
        if (!ready)
            return;
        if (live && !draft && !failed)
            run("read");
        else if (!live && preview)
            run("stop-preview");
    }
    Component.onCompleted: {
        ready = true;
        if (live)
            run("read");
    }
    Component.onDestruction: {
        // Dash may unload the tab before its stop request returns.
        if (preview || operation === "preview")
            Quickshell.execDetached([executable, "--control", "stop-preview"]);
    }

    Process {
        id: request
        stdout: StdioCollector {
            id: requestOutput
        }
        stderr: StdioCollector {
            id: requestError
        }
        onExited: (code, status) => session.finish(code, status)
    }
}
