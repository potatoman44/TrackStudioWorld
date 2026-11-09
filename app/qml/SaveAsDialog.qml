pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import TrackStudioWorld

// Save the open course into the mod as another course file, replacing that course there.
Kirigami.Dialog {
    id: dlg
    QQC2.Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.45)
    }
    required property Editor editor
    // the save result, for the Save dialog
    signal saved(var result)
    title: "Save As (Replace Course)"
    preferredWidth: Kirigami.Units.gridUnit * 42
    preferredHeight: Kirigami.Units.gridUnit * 34
    padding: Kirigami.Units.smallSpacing
    footer: DialogFooter {
        acceptText: "Replace and Save"
        acceptIcon: "document-save-as"
        acceptEnabled: courses.currentFile.length > 0 && modName.acceptableInput
        onAccepted: dlg.accept()
        onRejected: dlg.reject()
    }
    onOpened: {
        modName.text = editor.modName;
        courses.refresh(editor.saveAsDefault());
        courses.focusSearch();
    }
    onAccepted: replace(courses.currentFile)

    function replace(file) {
        if (file.length === 0)
            return;
        dlg.close();
        if (modName.text !== editor.modName)
            editor.modName = modName.text;
        dlg.saved(JSON.parse(editor.saveAs(file)));
    }

    ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: "Pick the course this one replaces. It is saved into the mod under that course's file name (with its own path data), so in game it runs wherever that course is chosen. The game loads the scenery around the replaced course, so build near it."
        }
        RowLayout {
            QQC2.Label { text: "Mod:" }
            QQC2.TextField {
                id: modName
                Layout.preferredWidth: Kirigami.Units.gridUnit * 14
                validator: RegularExpressionValidator {
                    regularExpression: /[A-Za-z0-9 _.\-]+/
                }
            }
        }
        CourseList {
            id: courses
            editor: dlg.editor
            Layout.fillWidth: true
            Layout.fillHeight: true
            onActivated: (file) => dlg.replace(file)
        }
    }
}
