pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import TrackStudioWorld

// Import an OBJ, FBX or COLLADA model into the open mod as a new object type (Assets ->
// Custom). The model gets the game's materials and lighting from a shipped object; collision is
// not imported yet.
Kirigami.Dialog {
    id: dlg
    QQC2.Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.45)
    }
    required property Editor editor
    property string error: ""
    property var info: ({})
    // the new object type, once imported
    signal imported(string actor)

    title: "Import Model"
    preferredWidth: Kirigami.Units.gridUnit * 30
    padding: Kirigami.Units.largeSpacing
    footer: DialogFooter {
        acceptText: "Import"
        acceptIcon: "document-import"
        acceptEnabled: dlg.info.ok === true && name.acceptableInput && !dlg.editor.busy
        onAccepted: dlg.accept()
        onRejected: dlg.reject()
    }

    // names of the custom objects already in the mod
    property var taken: []
    readonly property bool replacing: taken.indexOf(name.text) >= 0
    onOpened: {
        error = "";
        taken = JSON.parse(editor.customObjects()).map((o) => o.name);
    }
    function inspect() {
        info = file.text.length > 0 ? JSON.parse(editor.inspectModel(file.text)) : {};
        if (info.ok === true) {
            // a free name: "Untitled", "Untitled2", ...
            if (name.text.length === 0) {
                let n = info.name, k = 2;
                while (taken.indexOf(n) >= 0)
                    n = info.name.substring(0, 22) + (k++);
                name.text = n;
            }
            if (label.text.length === 0)
                label.text = info.label;
        }
    }
    function fmt(v) {
        return (v * scale.realValue).toFixed(1);
    }

    onAccepted: {
        const r = JSON.parse(editor.importModel(file.text, name.text, label.text, scale.realValue));
        if (r.ok === true) {
            dlg.imported(r.actor);
            file.text = "";
            name.text = "";
            label.text = "";
            info = {};
        } else {
            error = r.error;
            open();
        }
    }

    FileDialog {
        id: picker
        title: "Choose a model"
        nameFilters: ["3D models (*.obj *.fbx *.dae)", "Wavefront OBJ (*.obj)", "FBX (*.fbx)", "COLLADA (*.dae)"]
        // "file:///C:/x" on Windows: keep the drive letter at the start
        onAccepted: {
            file.text = decodeURIComponent(selectedFile.toString()).replace(/^file:\/\//, "").replace(/^\/([A-Za-z]:)/, "$1");
            name.text = "";
            label.text = "";
            dlg.inspect();
        }
    }

    ColumnLayout {
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: dlg.error.length > 0
            type: Kirigami.MessageType.Error
            text: dlg.error
        }
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: dlg.replacing
            type: Kirigami.MessageType.Warning
            text: "The mod already has an object called \"" + name.text + "\". Importing replaces it (and every placed copy of it). Change the name to add a new one."
        }
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: dlg.info.ok === false
            type: Kirigami.MessageType.Warning
            text: "Can't read this model: " + (dlg.info.error || "")
        }
        Kirigami.FormLayout {
            Layout.fillWidth: true
            RowLayout {
                Kirigami.FormData.label: "Model file:"
                QQC2.TextField {
                    id: file
                    Layout.minimumWidth: Kirigami.Units.gridUnit * 16
                    placeholderText: "OBJ, FBX or DAE"
                    onEditingFinished: dlg.inspect()
                }
                QQC2.Button {
                    icon.name: "folder-open"
                    onClicked: picker.open()
                    QQC2.ToolTip.text: "Browse…"
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                }
            }
            QQC2.Label {
                Kirigami.FormData.label: "Contains:"
                visible: dlg.info.ok === true
                text: dlg.info.ok === true
                    ? dlg.info.parts + " material(s), " + dlg.info.triangles + " triangles"
                    : ""
            }
            QQC2.Label {
                Kirigami.FormData.label: "Size:"
                visible: dlg.info.ok === true
                text: dlg.info.ok === true
                    ? dlg.fmt(dlg.info.size[0]) + " × " + dlg.fmt(dlg.info.size[1]) + " × " + dlg.fmt(dlg.info.size[2]) + " units"
                    : ""
            }
            QQC2.TextField {
                id: name
                Kirigami.FormData.label: "Name:"
                placeholderText: "letters and digits"
                maximumLength: 24
                validator: RegularExpressionValidator {
                    regularExpression: /[A-Za-z][A-Za-z0-9]{0,23}/
                }
                QQC2.ToolTip.text: "The object type becomes CmnObj_Custom<Name>000_A_00. Importing the same name again replaces it."
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
            QQC2.TextField {
                id: label
                Kirigami.FormData.label: "Label:"
                placeholderText: "shown in Assets"
            }
            QQC2.SpinBox {
                id: scale
                Kirigami.FormData.label: "Scale:"
                from: 1
                to: 1000000
                value: 1000
                stepSize: 100
                editable: true
                readonly property real realValue: value / 1000
                textFromValue: (v) => "× " + (v / 1000).toString()
                valueFromText: (t) => Math.round(parseFloat(t.replace("×", "")) * 1000)
            }
            QQC2.Label {
                Kirigami.FormData.label: "Mod:"
                text: dlg.editor.modName
            }
        }
        QQC2.Label {
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 26
            wrapMode: Text.Wrap
            opacity: 0.7
            text: "The model is written into the mod as a new object, listed in Assets under Custom. It is drawn with the game's own materials and baked lighting from a shipped object, using each material's colour texture; up to 6 materials (ones with the same texture are merged). A game unit is a metre. FBX and COLLADA files that say their unit are converted to metres; OBJ files have no unit, so set the scale if yours was made in another one (× 0.01 for centimetres). Collision is not imported yet: karts drive through imported objects."
        }
    }
}
