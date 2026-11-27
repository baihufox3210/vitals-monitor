pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: page
    required property var session
    required property Flickable viewport
    readonly property int rowHeight: 44
    spacing: 8

    Repeater {
        id: entries
        model: page.session.blacklist
        delegate: HudControls.Card {
            id: entryRow
            required property int index
            required property string name
            required property bool active
            property alias field: nameField
            flat: true
            RowLayout {
                Layout.fillWidth: true
                Layout.minimumHeight: page.rowHeight
                spacing: 8
                HudControls.Input {
                    id: nameField
                    text: entryRow.name
                    placeholderText: "執行檔名稱"
                    Accessible.name: "執行檔名稱"
                    onTextEdited: {
                        page.session.blacklist.setProperty(entryRow.index, "name", text);
                        page.session.dirty = true;
                    }
                }
                HudControls.Switch {
                    checked: entryRow.active
                    Accessible.name: "排除此程序"
                    onToggled: {
                        page.session.blacklist.setProperty(entryRow.index, "active", checked);
                        page.session.dirty = true;
                    }
                }
                HudControls.Action {
                    text: "移除"
                    onClicked: {
                        page.session.blacklist.remove(entryRow.index);
                        page.session.dirty = true;
                    }
                }
            }
            HudControls.Rule {}
        }
    }
    HudControls.Action {
        Layout.alignment: Qt.AlignLeft
        objectName: "addProgram"
        text: "＋ 新增程序"
        pill: true
        emphasis: 1
        onClicked: {
            page.session.blacklist.append({
                name: "",
                active: true
            });
            page.session.dirty = true;
            entries.itemAt(page.session.blacklist.count - 1)?.field.forceActiveFocus();
            Qt.callLater(() => {
                page.viewport.contentY = Math.max(0, page.viewport.contentHeight - page.viewport.height);
            });
        }
    }
}
