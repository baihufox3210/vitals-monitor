pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic as Controls
import QtQuick.Layouts
import qs.Common
import qs.Modules.Plugins

DashTabComponent {
    id: root

    readonly property var sections: ["Vitals", "位置", "資料來源", "自訂樣式", "黑名單"]
    property int page: 0
    property var collapsedMetrics: ({})
    readonly property int dragFrom: page === 0 ? (pageLoader?.item?.dragFrom ?? -1) : -1
    readonly property int dropSlot: page === 0 ? (pageLoader?.item?.dropSlot ?? -1) : -1
    readonly property alias session: hudSession
    implicitHeight: 520
    focusTarget: session.draft ? navigation.itemAt(page) : (session.failed ? retryButton : null)
    clip: true

    HudSession {
        id: hudSession
        live: root.live
        onTargetRequired: root.page = 2
    }
    onLiveChanged: {
        if (live)
            return;
        if (page === 0)
            pageLoader?.item?.endDrag?.(false);
        collapsedMetrics = {};
    }

    readonly property int inset: 16
    readonly property int scrollGutter: 8
    readonly property int rowHeight: 44
    readonly property real railWidth: Math.max(0, Math.min(720, width - 2 * inset))
    readonly property color ruleColor: Theme.withAlpha(Theme.surfaceText, 0.075)

    ColumnLayout {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.topMargin: 12
        anchors.bottomMargin: 10
        width: root.railWidth
        spacing: 8

        RowLayout {
            objectName: "sharedHeader"
            Layout.fillWidth: true
            Layout.minimumHeight: root.rowHeight
            spacing: 12

            HudControls.Label {
                Layout.fillWidth: true
                text: "Vitals Monitor"
                font.pixelSize: 22 * Theme.fontScale
                font.weight: Font.DemiBold
                font.letterSpacing: -0.4
                elide: Text.ElideRight
            }
            HudControls.Switch {
                objectName: "hudSwitch"
                Accessible.name: "啟用 Vitals Monitor"
                enabled: !!root.session.draft && !root.session.busy && root.dragFrom < 0
                checked: root.session.draft?.overlay ?? false
                onToggled: root.session.patch("overlay", checked)
            }
        }

        Rectangle {
            id: tabRail
            objectName: "tabRail"
            Layout.fillWidth: true
            readonly property int tabHeight: 34
            readonly property int tabInset: 3
            implicitHeight: tabHeight + 2 * tabInset
            Layout.minimumHeight: implicitHeight
            Layout.maximumHeight: implicitHeight
            radius: height / 2
            color: Theme.withAlpha(Theme.surfaceText, 0.055)
            border.color: root.ruleColor
            border.width: 1
            enabled: !!root.session.draft && !root.session.busy && root.dragFrom < 0

            Rectangle {
                objectName: "tabSelection"
                readonly property Item selectedTab: navigation.itemAt(root.page)
                x: tabs.x + (selectedTab?.x ?? 0)
                y: tabs.y
                width: selectedTab?.width ?? 0
                height: tabRail.tabHeight
                radius: height / 2
                visible: !!selectedTab
                color: Theme.isLightMode ? Theme.withAlpha("white", 0.85) : Theme.withAlpha(Theme.surfaceText, 0.135)
                border.color: Theme.withAlpha(Theme.surfaceText, 0.11)
                border.width: 1
                Behavior on x {
                    NumberAnimation {
                        duration: 170
                        easing.type: Easing.OutCubic
                    }
                }
                Behavior on width {
                    NumberAnimation {
                        duration: 170
                        easing.type: Easing.OutCubic
                    }
                }
            }
            RowLayout {
                id: tabs
                anchors.fill: parent
                anchors.margins: tabRail.tabInset
                spacing: 2

                Repeater {
                    id: navigation
                    model: root.sections
                    delegate: Controls.Button {
                        id: tabButton
                        required property int index
                        required property string modelData
                        readonly property bool selected: root.page === index
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        Layout.preferredWidth: 1
                        Layout.fillHeight: true
                        Layout.minimumHeight: 0
                        hoverEnabled: true
                        Accessible.role: Accessible.PageTab
                        Accessible.selected: selected

                        leftPadding: 2
                        rightPadding: 2
                        text: modelData
                        contentItem: HudControls.Label {
                            text: tabButton.text
                            color: tabButton.selected ? Theme.surfaceText : Theme.surfaceVariantText
                            font.weight: tabButton.selected ? Font.DemiBold : Font.Medium
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                        }
                        background: Rectangle {
                            anchors.fill: parent
                            radius: height / 2
                            color: !tabButton.enabled || tabButton.selected ? "transparent" : Theme.withAlpha(Theme.surfaceText, tabButton.down ? 0.14 : tabButton.hovered ? 0.07 : 0)
                            border.width: tabButton.visualFocus ? 1 : 0
                            border.color: Theme.primary
                            Behavior on color { ColorAnimation { duration: 150 } }
                        }
                        onClicked: root.page = index
                    }
                }
            }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 0

            ColumnLayout {
                anchors.centerIn: parent
                width: Math.min(parent.width, 340)
                visible: !root.session.draft
                spacing: 10

                HudControls.Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    text: root.session.failed ? "無法載入設定" : "正在讀取 Vitals Monitor 設定…"
                    font.weight: Font.DemiBold
                }
                HudControls.Label {
                    Layout.fillWidth: true
                    visible: root.session.failed
                    text: root.session.message
                    muted: true
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                }
                HudControls.Action {
                    id: retryButton
                    emphasis: 1
                    Layout.alignment: Qt.AlignHCenter
                    visible: root.session.failed
                    text: "重試"
                    onClicked: root.session.run("read")
                }
            }

            Flickable {
                id: scroller
                anchors.fill: parent
                visible: !!root.session.draft
                clip: true
                interactive: root.dragFrom < 0
                boundsBehavior: Flickable.StopAtBounds
                flickableDirection: Flickable.VerticalFlick
                contentWidth: width
                contentHeight: pageLoader.height
                Controls.ScrollBar.vertical: Controls.ScrollBar {
                    parent: scroller.parent
                    anchors.left: scroller.right
                    anchors.right: undefined
                    anchors.leftMargin: 2
                    anchors.top: scroller.top
                    anchors.bottom: scroller.bottom
                    width: root.scrollGutter - 2
                    policy: Controls.ScrollBar.AsNeeded
                }

                Loader {
                    id: pageLoader
                    width: scroller.width
                    active: scroller.visible
                    enabled: !root.session.busy
                    sourceComponent: [metricsPage, placementPage, sourcesPage, stylePage, blacklistPage][root.page]
                    onLoaded: scroller.contentY = 0
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            visible: !!root.session.draft
            enabled: root.dragFrom < 0
            spacing: 6

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 1
                color: root.ruleColor
            }
            HudControls.Label {
                Layout.fillWidth: true
                visible: root.session.busy || root.session.failed
                text: root.session.busy ? "處理中…" : root.session.message
                color: root.session.failed ? Theme.error : Theme.surfaceVariantText
                wrapMode: Text.Wrap
                maximumLineCount: 3
                elide: Text.ElideRight
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight
                spacing: 6
                HudControls.Action {
                    visible: root.session.preview
                    text: "關閉預覽"
                    enabled: !root.session.busy
                    onClicked: root.session.run("stop-preview")
                }
                HudControls.Action {
                    text: root.session.preview ? "更新預覽" : "預覽"
                    emphasis: 1
                    enabled: !root.session.busy
                    onClicked: root.session.run("preview")
                }
                HudControls.Action {
                    text: "套用"
                    emphasis: root.session.dirty ? 2 : 1
                    enabled: !root.session.busy
                    onClicked: root.session.run("apply")
                }
            }
        }
    }

    Component {
        id: metricsPage
        MetricsPage {
            session: root.session
            viewport: scroller
            collapsedMetrics: root.collapsedMetrics
            onCollapseRequested: (metric, collapsed) => {
                root.collapsedMetrics = Object.assign({}, root.collapsedMetrics, {
                    [metric]: collapsed
                });
            }
        }
    }
    Component {
        id: placementPage
        PlacementPage {
            session: root.session
            railWidth: root.railWidth
        }
    }
    Component {
        id: sourcesPage
        SourcesPage {
            session: root.session
            railWidth: root.railWidth
        }
    }
    Component {
        id: stylePage
        StylePage {
            session: root.session
            viewport: scroller
        }
    }
    Component {
        id: blacklistPage
        BlacklistPage {
            session: root.session
            viewport: scroller
        }
    }
}
