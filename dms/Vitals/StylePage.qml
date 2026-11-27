pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import qs.Common

ColumnLayout {
    id: page
    required property var session
    required property Flickable viewport
    spacing: 8

    Controls.TextArea {
        id: editor
        objectName: "cssEditor"
        Layout.fillWidth: true
        Layout.margins: 3
        Layout.minimumHeight: Math.max(120, page.viewport.height - 6)
        padding: 14
        selectionColor: Theme.primary
        selectedTextColor: Theme.primaryText
        text: page.session.draft?.css ?? ""
        color: Theme.surfaceText
        placeholderTextColor: Theme.surfaceVariantText
        placeholderText: ".hud .metric-value { color: #f1f5fa; }"
        font.family: "monospace"
        font.pixelSize: Math.max(9, Math.round(11 * Theme.fontScale))
        wrapMode: TextEdit.Wrap
        selectByMouse: true
        Accessible.name: "自訂 GTK CSS"
        background: HudControls.FieldBackground {
            focused: editor.activeFocus
            blueFocus: true
        }
        onTextChanged: if (activeFocus)
            page.session.patch("css", text)
        onCursorRectangleChanged: {
            if (!activeFocus)
                return;
            const y = mapToItem(page.viewport.contentItem, 0, cursorRectangle.y).y;
            if (y < page.viewport.contentY)
                page.viewport.contentY = y;
            else if (y + cursorRectangle.height > page.viewport.contentY + page.viewport.height)
                page.viewport.contentY = y + cursorRectangle.height - page.viewport.height;
        }
    }
}
