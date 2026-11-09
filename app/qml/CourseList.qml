pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import TrackStudioWorld

// A searchable list of the game's course files, grouped by kind (as in Open Course). The
// current course is `currentFile`; double-click or Enter sends `activated`.
ColumnLayout {
    id: cl
    required property Editor editor
    property var files: []
    readonly property string currentFile: list.currentIndex >= 0 && list.currentIndex < files.length ? files[list.currentIndex].file : ""
    signal activated(string file)
    spacing: Kirigami.Units.smallSpacing

    // fill the list and select `select` (a course file) when it is in it
    function refresh(select) {
        files = JSON.parse(editor.courseList(search.text));
        let i = files.length > 0 ? 0 : -1;
        if (select && select.length > 0) {
            for (let k = 0; k < files.length; k++) {
                if (files[k].file === select) {
                    i = k;
                    break;
                }
            }
        }
        list.currentIndex = i;
        list.positionViewAtIndex(Math.max(i, 0), ListView.Center);
    }
    function focusSearch() {
        search.forceActiveFocus();
    }

    Kirigami.SearchField {
        id: search
        Layout.fillWidth: true
        placeholderText: "Search (FirstCircuit, Race_, Rainbow, 荒野…)"
        autoAccept: false
        onTextChanged: cl.refresh("")
        onAccepted: if (cl.currentFile.length > 0) cl.activated(cl.currentFile)
        Keys.onDownPressed: list.incrementCurrentIndex()
        Keys.onUpPressed: list.decrementCurrentIndex()
    }
    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 22
        ListView {
            id: list
            clip: true
            model: cl.files
            section.property: "group"
            section.delegate: Kirigami.ListSectionHeader {
                required property string section
                text: section
                width: ListView.view.width
            }
            highlightMoveDuration: 0
            keyNavigationEnabled: true
            delegate: QQC2.ItemDelegate {
                id: d
                required property var modelData
                required property int index
                width: ListView.view.width
                highlighted: ListView.isCurrentItem
                background: Rectangle {
                    readonly property color hl: Kirigami.Theme.highlightColor
                    color: d.highlighted ? hl : d.hovered ? Qt.rgba(hl.r, hl.g, hl.b, 0.15) : "transparent"
                }
                contentItem: RowLayout {
                    QQC2.Label { text: d.modelData.name; color: d.highlighted ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor; Layout.preferredWidth: Kirigami.Units.gridUnit * 18; elide: Text.ElideRight }
                    QQC2.Label { text: d.modelData.display; color: d.highlighted ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.disabledTextColor; Layout.fillWidth: true; elide: Text.ElideRight }
                }
                onClicked: list.currentIndex = index
                onDoubleClicked: cl.activated(modelData.file)
            }
        }
    }
}
