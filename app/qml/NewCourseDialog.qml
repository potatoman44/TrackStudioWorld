pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import TrackStudioWorld

// Start a new course where a shipped course is (its stage is drawn around it), empty or as a
// copy of that course. Saving it asks which course it replaces (Save As).
Kirigami.Dialog {
    id: dlg
    QQC2.Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.45)
    }
    required property Editor editor
    // called with the start as a function, to ask about unsaved changes first
    property var guard: null
    title: "New Course"
    preferredWidth: Kirigami.Units.gridUnit * 42
    preferredHeight: Kirigami.Units.gridUnit * 34
    padding: Kirigami.Units.smallSpacing
    footer: DialogFooter {
        acceptText: "Create"
        acceptIcon: "document-new"
        acceptEnabled: courses.currentFile.length > 0
        onAccepted: dlg.accept()
        onRejected: dlg.reject()
    }
    onOpened: {
        courses.refresh("");
        courses.focusSearch();
    }
    onAccepted: start(courses.currentFile)

    function start(file) {
        if (file.length === 0)
            return;
        dlg.close();
        const copy = copyOption.checked;
        const go = () => dlg.editor.newCourse(file, copy);
        if (dlg.guard)
            dlg.guard(go);
        else
            go();
    }

    ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: "Where to build: the new course starts at this course's place in the world, with the stage around it drawn. When you save, you pick which course it replaces in the mod; the game loads the scenery around that course, so replacing the one you build at is safest."
        }
        CourseList {
            id: courses
            editor: dlg.editor
            Layout.fillWidth: true
            Layout.fillHeight: true
            onActivated: (file) => dlg.start(file)
        }
        RowLayout {
            QQC2.RadioButton {
                id: emptyOption
                text: "Empty (no objects or paths)"
                checked: true
            }
            QQC2.RadioButton {
                id: copyOption
                text: "Copy of this course's objects and paths"
            }
        }
    }
}
