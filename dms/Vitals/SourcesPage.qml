pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: page
    required property var session
    required property real railWidth
    spacing: 0
    HudControls.SettingRow {
        controlWidth: page.railWidth * 0.58
        title: "遊戲執行檔"
        HudControls.Input {
            session: page.session
            setting: "target"
            Layout.maximumWidth: 480
            Accessible.name: "遊戲執行檔"
        }
    }
    HudControls.SettingRow {
        controlWidth: page.railWidth * 0.58
        title: "NGX 影格日誌"
        HudControls.Input {
            session: page.session
            setting: "timing_path"
            Layout.maximumWidth: 480
            Accessible.name: "NGX 影格日誌"
        }
    }
    HudControls.SettingRow {
        controlWidth: page.railWidth * 0.58
        title: "GNUMON CLI"
        HudControls.Input {
            session: page.session
            setting: "gnumon"
            Layout.maximumWidth: 480
            Accessible.name: "GNUMON CLI"
        }
    }
}
