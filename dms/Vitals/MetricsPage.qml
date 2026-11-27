pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import QtQuick.Shapes
import qs.Common

Item {
    id: metrics
    required property var session
    required property Flickable viewport
    required property var collapsedMetrics
    signal collapseRequested(string metric, bool collapsed)
    readonly property int rowHeight: 44
    readonly property var metricNames: ({
            gpu: "GPU",
            vram: "VRAM",
            cpu: "CPU",
            ram: "RAM",
            game_proc: "遊戲程序",
            disk: "磁碟 / NVMe",
            network: "網路流量",
            fps: "FPS",
            frame_stats: "影格穩定度",
            dlss: "DLSS / 插幀",
            frame_time: "影格時間"
        })
    readonly property var fieldNames: ({
            "gpu.util": "使用率",
            "gpu.temp": "溫度",
            "gpu.clock": "核心時脈",
            "gpu.power": "功耗",
            "gpu.fan": "風扇轉速",
            "vram.used": "已用 / 總容量",
            "vram.clock": "記憶體時脈",
            "cpu.usage": "使用率",
            "cpu.temp": "溫度",
            "cpu.clock": "最高時脈",
            "ram.used": "已用 / 總容量",
            "ram.swap": "Swap 用量",
            "game_proc.cpu": "遊戲 CPU%",
            "game_proc.ram": "遊戲記憶體",
            "game_proc.threads": "執行緒數",
            "game_proc.uptime": "運行時間",
            "disk.read": "讀取速率",
            "disk.write": "寫入速率",
            "disk.temp": "NVMe 溫度",
            "network.rx": "下載速率",
            "network.tx": "上傳速率",
            "fps.native": "原生 FPS",
            "fps.output": "輸出 FPS",
            "frame_stats.p50": "P50 中位數",
            "frame_stats.p95": "P95 幀率",
            "frame_stats.low": "低點 FPS",
            "dlss.sr": "SR 耗時",
            "dlss.fg": "FG 耗時與倍率",
            "dlss.res": "解析度縮放",
            "frame_time.avg": "平均影格時間",
            "frame_time.graph": "30 秒波形圖",
            "frame_time.p95": "P95 時間",
            "frame_time.peak": "尖峰時間"
        })
    property int dragFrom: -1
    property int dropSlot: -1
    property real dragY: 0
    function endDrag(commit) {
        const from = dragFrom;
        const slot = dropSlot;
        dragFrom = -1;
        dropSlot = -1;
        if (commit && from >= 0 && slot >= 0)
            session.moveMetric(from, slot > from ? slot - 1 : slot);
    }

    implicitHeight: rows.implicitHeight

    function updateDrop() {
        const y = metrics.mapFromItem(metrics.viewport, 0, metrics.dragY).y;
        let slot = tiles.count;
        for (let i = 0; i < tiles.count; ++i) {
            const row = tiles.itemAt(i);
            if (row && y < row.y + row.height / 2) {
                slot = i;
                break;
            }
        }
        metrics.dropSlot = slot;
    }

    Timer {
        interval: 16
        repeat: true
        running: metrics.dragFrom >= 0
        onTriggered: {
            const edge = Math.min(54, metrics.viewport.height / 3);
            if (edge <= 0)
                return;
            const step = metrics.dragY < edge ? Math.max(-18, (metrics.dragY - edge) / 3) : metrics.dragY > metrics.viewport.height - edge ? Math.min(18, (metrics.dragY - metrics.viewport.height + edge) / 3) : 0;
            if (step !== 0) {
                metrics.viewport.contentY = Math.max(0, Math.min(metrics.viewport.contentHeight - metrics.viewport.height, metrics.viewport.contentY + step));
                metrics.updateDrop();
            }
        }
    }

    ColumnLayout {
        id: rows
        width: parent.width
        spacing: 0
        Repeater {
            id: tiles
            model: metrics.session.draft?.tiles.length ?? 0
            delegate: HudControls.Card {
                id: row
                required property int index
                readonly property var tile: metrics.session.draft.tiles[index]
                readonly property string label: metrics.metricNames[tile.metric] || tile.metric
                readonly property bool hasDetails: tile.enabled && tile.fields.length > 0
                readonly property bool detailsOpen: hasDetails && metrics.collapsedMetrics[tile.metric] === false
                flat: true
                opacity: metrics.dragFrom === index ? 0.38 : 1

                RowLayout {
                    Layout.fillWidth: true
                    Layout.minimumHeight: metrics.rowHeight
                    spacing: 8
                    Item {
                        id: grip
                        Layout.preferredWidth: 34
                        Layout.preferredHeight: 36
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: "排序：" + row.label
                        Accessible.description: "拖曳，或按上、下鍵調整順序"
                        Keys.onUpPressed: {
                            metrics.session.moveMetric(row.index, row.index - 1);
                            if (row.index > 0)
                                tiles.itemAt(row.index - 1)?.sortGrip.forceActiveFocus();
                        }
                        Keys.onDownPressed: {
                            metrics.session.moveMetric(row.index, row.index + 1);
                            if (row.index + 1 < tiles.count)
                                tiles.itemAt(row.index + 1)?.sortGrip.forceActiveFocus();
                        }
                        Rectangle {
                            anchors.fill: parent
                            radius: 10
                            color: pointer.containsMouse || grip.activeFocus ? Theme.withAlpha(Theme.surfaceText, 0.08) : "transparent"
                        }
                        HudControls.GripDots {
                            anchors.centerIn: parent
                        }
                        MouseArea {
                            id: pointer
                            anchors.fill: parent
                            hoverEnabled: true
                            preventStealing: true
                            cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                            property real pressY: 0
                            onPressed: mouse => {
                                pressY = mapToItem(metrics.viewport, 0, mouse.y).y;
                                metrics.viewport.cancelFlick();
                            }
                            onPositionChanged: mouse => {
                                if (!pressed)
                                    return;
                                const y = mapToItem(metrics.viewport, 0, mouse.y).y;
                                if (metrics.dragFrom < 0 && Math.abs(y - pressY) < Qt.styleHints.startDragDistance)
                                    return;
                                metrics.dragFrom = row.index;
                                metrics.dragY = y;
                                metrics.updateDrop();
                            }
                            onReleased: metrics.endDrag(true)
                            onCanceled: metrics.endDrag(false)
                        }
                    }
                    HudControls.Action {
                        id: metricTitle
                        objectName: "metricTitle"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.minimumWidth: 0
                        expandedTint: row.detailsOpen
                        text: row.label
                        enabled: row.hasDetails && metrics.dragFrom < 0
                        contentItem: HudControls.Label {
                            text: metricTitle.text
                            muted: !row.tile.enabled
                            font.weight: Font.DemiBold
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                        }
                        Accessible.description: row.detailsOpen ? "收合細項" : "展開細項"
                        rightPadding: row.hasDetails ? 29 : 12
                        Shape {
                            anchors.right: parent.right
                            anchors.rightMargin: 10
                            anchors.verticalCenter: parent.verticalCenter
                            width: 10
                            height: 10
                            visible: row.hasDetails
                            rotation: row.detailsOpen ? 0 : -90
                            Behavior on rotation {
                                NumberAnimation {
                                    duration: 180
                                    easing.type: Easing.OutCubic
                                }
                            }
                            ShapePath {
                                strokeWidth: 1.4
                                strokeColor: Theme.surfaceVariantText
                                fillColor: "transparent"
                                capStyle: ShapePath.RoundCap
                                joinStyle: ShapePath.RoundJoin
                                startX: 2
                                startY: 4
                                PathLine {
                                    x: 5
                                    y: 7
                                }
                                PathLine {
                                    x: 8
                                    y: 4
                                }
                            }
                        }
                        onClicked: metrics.collapseRequested(row.tile.metric, row.detailsOpen)
                    }
                    HudControls.Switch {
                        checked: row.tile.enabled
                        enabled: metrics.dragFrom < 0
                        Accessible.name: "顯示 " + row.label
                        onToggled: metrics.session.updateMetric(row.index, -1, checked)
                    }
                }
                Item {
                    Layout.fillWidth: true
                    Layout.leftMargin: 42
                    Layout.preferredHeight: reveal * detailRows.implicitHeight
                    visible: reveal > 0
                    clip: true
                    enabled: row.detailsOpen && metrics.dragFrom < 0
                    property real reveal: row.detailsOpen ? 1 : 0
                    Behavior on reveal {
                        NumberAnimation {
                            duration: 190
                            easing.type: Easing.InOutCubic
                        }
                    }
                    ColumnLayout {
                        id: detailRows
                        width: parent.width
                        spacing: 0
                        opacity: parent.reveal
                        Repeater {
                            model: row.tile.fields.length
                            delegate: HudControls.Toggle {
                                required property int index
                                readonly property var field: row.tile.fields[index]
                                Layout.fillWidth: true
                                text: field ? (metrics.fieldNames[row.tile.metric + "." + field.id] || field.id) : ""
                                checked: field?.enabled ?? false
                                onToggled: if (field)
                                    metrics.session.updateMetric(row.index, index, checked)
                            }
                        }
                    }
                }
                HudControls.Rule {
                    Layout.leftMargin: 42
                }
                property alias sortGrip: grip
            }
        }
    }

    Rectangle {
        width: parent.width
        height: 2
        radius: 1
        z: 1
        color: Theme.primary
        visible: metrics.dragFrom >= 0 && metrics.dropSlot >= 0 && metrics.dropSlot !== metrics.dragFrom && metrics.dropSlot !== metrics.dragFrom + 1
        y: metrics.dropSlot === tiles.count ? rows.height - 2 : Math.max(0, (tiles.itemAt(metrics.dropSlot)?.y ?? 0) - 1)
    }

    HudControls.Surface {
        width: Math.max(0, Math.min(240, parent.width - 16))
        color: Theme.surfaceContainerHigh
        height: 36
        x: parent.width - width - 8
        y: metrics.viewport.contentY + Math.max(2, Math.min(metrics.viewport.height - height - 2, metrics.dragY - height / 2))
        z: 2
        visible: metrics.dragFrom >= 0
        border.color: Theme.withAlpha(Theme.primary, 0.5)
        Row {
            anchors.centerIn: parent
            spacing: 8
            HudControls.GripDots {
                anchors.verticalCenter: parent.verticalCenter
            }
            HudControls.Label {
                readonly property string metric: metrics.session.draft?.tiles[metrics.dragFrom]?.metric ?? ""
                text: metrics.metricNames[metric] || metric
                font.weight: Font.DemiBold
            }
        }
    }
}
