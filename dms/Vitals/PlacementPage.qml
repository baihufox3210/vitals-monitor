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
        title: "對齊角落"
        RowLayout {
            Layout.preferredWidth: page.railWidth < 360 ? 148 : 200
            spacing: 2
            Repeater {
                model: ["左上", "右上", "左下", "右下"]
                delegate: HudControls.Action {
                    required property int index
                    required property string modelData
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    pill: true
                    implicitHeight: 30
                    leftPadding: 4
                    rightPadding: 4
                    text: modelData
                    emphasis: page.session.draft?.corner === index ? 1 : 0
                    Accessible.name: "對齊角落：" + text
                    onClicked: page.session.patch("corner", index)
                }
            }
        }
    }
    HudControls.SettingRow {
        controlWidth: page.railWidth * 0.58
        title: "欄數"
        HudControls.NumberInput {
            session: page.session
            setting: "columns"
            from: 1
            to: 3
            Accessible.name: "欄數"
        }
    }
    HudControls.SettingRow {
        controlWidth: page.railWidth * 0.58
        title: "寬度"
        HudControls.NumberInput {
            session: page.session
            setting: "width"
            from: 180
            to: 1000
            Accessible.name: "寬度"
        }
    }
    HudControls.SettingRow {
        controlWidth: page.railWidth * 0.58
        title: "水平邊距"
        HudControls.NumberInput {
            session: page.session
            setting: "x"
            from: 0
            to: 500
            Accessible.name: "水平邊距"
        }
    }
    HudControls.SettingRow {
        controlWidth: page.railWidth * 0.58
        title: "垂直邊距"
        HudControls.NumberInput {
            session: page.session
            setting: "y"
            from: 0
            to: 500
            Accessible.name: "垂直邊距"
        }
    }
    HudControls.SettingSwitch {
        session: page.session
        controlWidth: page.railWidth * 0.58
        title: "僅在遊戲運行時顯示"
        setting: "auto_show"
    }
    HudControls.SettingSwitch {
        session: page.session
        controlWidth: page.railWidth * 0.58
        title: "滑鼠移入時自動隱藏"
        setting: "hover_hide"
    }
    HudControls.SettingSwitch {
        session: page.session
        controlWidth: page.railWidth * 0.58
        title: "自動偵測遊戲"
        setting: "auto_detect"
    }
}
