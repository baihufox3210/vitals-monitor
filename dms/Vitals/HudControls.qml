import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import QtQuick.Shapes
import qs.Common

QtObject {
    component Label: Text {
        property bool muted: false
        textFormat: Text.PlainText
        color: muted ? Theme.surfaceVariantText : Theme.surfaceText
        font.pixelSize: 13 * Theme.fontScale
    }
    component Surface: Rectangle {
        property bool flat: false
        radius: 12
        color: flat ? "transparent" : Theme.withAlpha(Theme.surfaceText, 0.045)
        border.width: flat ? 0 : 1
        border.color: Theme.withAlpha(Theme.surfaceText, 0.08)
    }
    component Action: Controls.Button {
        id: action
        property int emphasis: 0
        property bool pill: true
        property bool expandedTint: false
        implicitHeight: 34
        leftPadding: 12
        rightPadding: 12
        contentItem: Label {
            text: action.text
            font.weight: Font.Medium
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            color: !action.enabled ? Theme.surfaceVariantText : action.emphasis === 2 ? Theme.primaryText : action.emphasis === 1 ? Theme.primary : Theme.surfaceText
        }
        background: Rectangle {
            implicitWidth: action.emphasis > 0 ? 64 : 0
            radius: action.pill ? height / 2 : 9
            color: action.emphasis === 2 && action.enabled ? (action.down ? Qt.darker(Theme.primary, 1.12) : Theme.primary) : Theme.withAlpha(action.emphasis === 1 ? Theme.primary : Theme.surfaceText, !action.enabled ? 0 : action.down ? 0.16 : action.hovered ? 0.09 : action.expandedTint || action.emphasis === 1 ? 0.055 : 0)
            border.width: action.visualFocus ? 1 : 0
            border.color: Theme.primary
            Behavior on color {
                ColorAnimation {
                    duration: 150
                }
            }
        }
    }
    component Card: Surface {
        default property alias body: cardBody.data
        Layout.fillWidth: true
        implicitHeight: cardBody.implicitHeight + 12
        ColumnLayout {
            id: cardBody
            anchors.fill: parent
            anchors.margins: 6
            anchors.leftMargin: 0
            anchors.rightMargin: 0
            spacing: 6
        }
    }
    component GripDots: Item {
        width: 9
        height: 15
        Repeater {
            model: 6
            delegate: Rectangle {
                required property int index
                x: (index % 2) * 6
                y: Math.floor(index / 2) * 6
                width: 3
                height: 3
                radius: 1.5
                color: Theme.surfaceVariantText
            }
        }
    }
    component Switch: Controls.Switch {
        id: control
        implicitWidth: 39
        implicitHeight: 32
        indicator: Rectangle {
            x: (control.width - width) / 2
            y: (control.height - height) / 2
            width: 34
            height: 20
            radius: height / 2
            opacity: control.enabled ? 1 : 0.55
            color: control.checked ? Theme.primary : Theme.withAlpha(Theme.surfaceText, control.hovered ? 0.19 : 0.13)
            border.width: 1
            border.color: Theme.withAlpha(Theme.surfaceText, control.checked ? 0.05 : 0.08)
            Behavior on color { ColorAnimation { duration: 140 } }
            Rectangle {
                anchors.fill: parent
                anchors.margins: -2
                radius: height / 2
                visible: control.visualFocus
                color: "transparent"
                border.color: Theme.primary
            }
            Rectangle {
                x: control.checked ? parent.width - width - 3 : 3
                anchors.verticalCenter: parent.verticalCenter
                width: 14
                height: 14
                radius: width / 2
                color: control.checked ? Theme.primaryText : (Theme.isLightMode ? "white" : Theme.surfaceText)
                border.color: Theme.withAlpha("black", 0.10)
                Behavior on color { ColorAnimation { duration: 140 } }
                Behavior on x {
                    NumberAnimation {
                        duration: 130
                        easing.type: Easing.OutCubic
                    }
                }
            }
        }
    }
    component FieldBackground: Rectangle {
        id: fieldSurface
        property bool focused: false
        property bool blueFocus: false
        readonly property color focusColor: blueFocus ? (Theme.isLightMode ? "#2869ce" : "#82b5ff") : Theme.primary
        radius: 12
        color: Theme.withAlpha(Theme.surfaceText, focused ? 0.065 : 0.04)
        border.color: focused ? focusColor : Theme.withAlpha(Theme.surfaceText, 0.12)
        Behavior on color { ColorAnimation { duration: 150 } }
        border.width: 1
        Behavior on border.color {
            ColorAnimation {
                duration: 150
            }
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: -2
            radius: fieldSurface.radius + 2
            color: "transparent"
            border.color: Theme.withAlpha(fieldSurface.focusColor, 0.24)
            opacity: fieldSurface.blueFocus && fieldSurface.focused ? 1 : 0
            Behavior on opacity {
                NumberAnimation {
                    duration: 160
                }
            }
        }
    }
    component Rule: Rectangle {
        Layout.fillWidth: true
        implicitHeight: 1
        color: Theme.withAlpha(Theme.surfaceText, 0.075)
    }
    component SettingRow: ColumnLayout {
        id: settingLine
        required property string title
        property real controlWidth: 420
        default property alias control: lineControls.data
        Layout.fillWidth: true
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.minimumHeight: 44
            spacing: 12
            Label {
                Layout.fillWidth: true
                Layout.minimumWidth: Math.min(104, implicitWidth)
                text: settingLine.title
                elide: Text.ElideRight
            }
            RowLayout {
                id: lineControls
                Layout.fillWidth: true
                Layout.maximumWidth: Math.min(480, settingLine.controlWidth)
                spacing: 0
                Item {
                    Layout.fillWidth: true
                }
            }
        }
        Rule {}
    }
    component SettingSwitch: SettingRow {
        id: settingRow
        required property var session
        required property string setting
        Switch {
            Accessible.name: settingRow.title
            checked: settingRow.session.draft?.[settingRow.setting] ?? false
            onToggled: settingRow.session.patch(settingRow.setting, checked)
        }
    }
    component Toggle: Controls.CheckBox {
        id: check
        implicitHeight: 30
        spacing: 10
        leftPadding: 12
        rightPadding: 12
        indicator: Rectangle {
            x: check.leftPadding
            anchors.verticalCenter: parent.verticalCenter
            implicitWidth: 16
            implicitHeight: 16
            radius: 5
            color: check.checked ? Theme.primary : Theme.withAlpha(Theme.surfaceText, check.hovered ? 0.10 : 0.035)
            border.width: 1
            border.color: Theme.withAlpha(Theme.surfaceText, check.checked ? 0.05 : 0.30)
            Behavior on color {
                ColorAnimation {
                    duration: 150
                }
            }
            Shape {
                anchors.centerIn: parent
                width: 12
                height: 12
                opacity: check.checked ? 1 : 0
                scale: check.checked ? 1 : 0.75
                Behavior on opacity {
                    NumberAnimation {
                        duration: 140
                    }
                }
                Behavior on scale {
                    NumberAnimation {
                        duration: 160
                        easing.type: Easing.OutCubic
                    }
                }
                ShapePath {
                    strokeWidth: 1.55
                    strokeColor: Theme.primaryText
                    fillColor: "transparent"
                    capStyle: ShapePath.RoundCap
                    joinStyle: ShapePath.RoundJoin
                    startX: 2.5
                    startY: 6
                    PathLine {
                        x: 5
                        y: 8.5
                    }
                    PathLine {
                        x: 9.5
                        y: 3.5
                    }
                }
            }
        }
        contentItem: Label {
            leftPadding: check.indicator.width + check.spacing
            text: check.text
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
        background: Rectangle {
            radius: 9
            color: Theme.withAlpha(Theme.surfaceText, check.down ? 0.10 : check.hovered ? 0.055 : 0)
            border.width: check.visualFocus ? 1 : 0
            border.color: Theme.primary
            Behavior on color { ColorAnimation { duration: 140 } }
        }
    }

    component NumberInput: Controls.SpinBox {
        id: numberInput
        required property var session
        required property string setting
        Layout.preferredWidth: 132
        objectName: "stepper_" + setting
        implicitHeight: 34
        padding: 2
        editable: true
        value: session.draft?.[setting] ?? from
        background: FieldBackground {
            radius: height / 2
            focused: numberInput.activeFocus
        }
        contentItem: TextInput {
            text: numberInput.displayText
            validator: numberInput.validator
            readOnly: !numberInput.editable
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            selectByMouse: true
            color: Theme.surfaceText
            selectionColor: Theme.primary
            selectedTextColor: Theme.primaryText
            font.pixelSize: 13 * Theme.fontScale
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
        down.indicator: StepIndicator {
            x: numberInput.mirrored ? numberInput.width - width - 3 : 3
            y: (numberInput.height - height) / 2
            pressed: numberInput.down.pressed
            hovered: numberInput.down.hovered
            increment: false
        }
        up.indicator: StepIndicator {
            x: numberInput.mirrored ? 3 : numberInput.width - width - 3
            y: (numberInput.height - height) / 2
            pressed: numberInput.up.pressed
            hovered: numberInput.up.hovered
            increment: true
        }
        onValueModified: session.patch(setting, value)
    }
    component StepIndicator: Rectangle {
        id: step
        required property bool pressed
        required property bool hovered
        required property bool increment
        implicitWidth: 28
        implicitHeight: 28
        radius: height / 2
        color: Theme.withAlpha(Theme.surfaceText, enabled ? (pressed ? 0.16 : hovered ? 0.11 : 0.055) : 0)
        Behavior on color { ColorAnimation { duration: 140 } }
        Rectangle {
            anchors.centerIn: parent
            width: 10
            height: 2
            radius: 1
            color: Theme.surfaceText
            opacity: step.enabled ? 1 : 0.36
        }
        Rectangle {
            anchors.centerIn: parent
            width: 2
            height: 10
            radius: 1
            visible: step.increment
            color: Theme.surfaceText
            opacity: step.enabled ? 1 : 0.36
        }
    }
    component Input: Controls.TextField {
        property var session: null
        property string setting: ""
        Layout.fillWidth: true
        Layout.minimumWidth: 0
        implicitHeight: 32
        padding: 8
        font.pixelSize: 13 * Theme.fontScale
        selectByMouse: true
        color: Theme.surfaceText
        placeholderTextColor: Theme.surfaceVariantText
        selectionColor: Theme.primary
        selectedTextColor: Theme.primaryText
        text: session?.draft?.[setting] ?? ""
        background: FieldBackground {
            focused: parent.activeFocus
            blueFocus: setting === ""
        }
        onTextEdited: if (session && setting !== "")
            session.patch(setting, text)
    }
}
