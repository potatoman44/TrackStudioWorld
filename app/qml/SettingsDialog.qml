pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import TrackStudioWorld

Kirigami.Dialog {
    id: dlg
    // dim the editor behind the dialog (set here: the default fade did not always end dimmed)
    QQC2.Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.45)
    }
    required property Editor editor
    title: "Settings"
    preferredWidth: Kirigami.Units.gridUnit * 40
    padding: Kirigami.Units.largeSpacing
    footer: DialogFooter {
        acceptText: "Save"
        acceptIcon: "document-save"
        onAccepted: dlg.accept()
        onRejected: dlg.reject()
    }
    property string error: ""
    // set while the dialog reopens to show an error from saving
    property bool reopening: false
    onOpened: {
        if (reopening) {
            reopening = false;
            return;
        }
        romfs.text = editor.romfsPath;
        mods.text = editor.modsPath;
        ns2jb.text = editor.ns2jbPath;
        // long paths show their start (where the folder is), not their end
        for (const f of [romfs, mods, ns2jb])
            f.cursorPosition = 0;
        ns2jbOnSave.checked = editor.ns2jbOnSave;
        const t = JSON.parse(editor.pref("testCommand"));
        testCommand.text = t ? t : "";
        error = "";
    }
    // accepting closes the dialog: on an error it opens again, keeping what was typed
    onAccepted: {
        editor.setPref("testCommand", JSON.stringify(testCommand.text));
        error = editor.setExport(ns2jb.text, ns2jbOnSave.checked);
        // changing the game folders reloads them; the export target alone does not
        if (error.length === 0 && (romfs.text !== editor.romfsPath || mods.text !== editor.modsPath))
            error = editor.applySettings(romfs.text, mods.text);
        if (error.length > 0) {
            reopening = true;
            dlg.open();
        }
    }
    FolderDialog {
        id: picker
        property var target
        // "file:///C:/x" on Windows: keep the drive letter at the start
        onAccepted: target.text = decodeURIComponent(selectedFolder.toString()).replace(/^file:\/\//, "").replace(/^\/([A-Za-z]:)/, "$1")
    }
    ColumnLayout {
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: dlg.error.length > 0
            type: Kirigami.MessageType.Error
            text: dlg.error
        }
        Kirigami.FormLayout {
            Layout.fillWidth: true
            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: "Game and mods"
            }
            RowLayout {
                Kirigami.FormData.label: "Game romfs:"
                QQC2.TextField { id: romfs; QQC2.ToolTip.text: text; QQC2.ToolTip.visible: hovered && text.length > 0; QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay; Layout.fillWidth: true; Layout.minimumWidth: Kirigami.Units.gridUnit * 24 }
                QQC2.Button { icon.name: "folder-open"; onClicked: { picker.target = romfs; picker.open(); } }
            }
            RowLayout {
                Kirigami.FormData.label: "Mods folder:"
                QQC2.TextField { id: mods; QQC2.ToolTip.text: text; QQC2.ToolTip.visible: hovered && text.length > 0; QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay; Layout.fillWidth: true }
                QQC2.Button { icon.name: "folder-open"; onClicked: { picker.target = mods; picker.open(); } }
            }
            QQC2.Label {
                Layout.fillWidth: true
                // a label's implicit width is its unwrapped text: cap it so the dialog keeps its width
                Layout.maximumWidth: Kirigami.Units.gridUnit * 30
                wrapMode: Text.Wrap
                opacity: 0.7
                text: "Mods are written to <mods folder>/<mod name>/romfs. Point the mods folder at ~/.local/share/eden/load/0400C3F00006E000 to save straight into the emulator."
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: "Switch 2 (ns2jb-modloader)"
            }
            RowLayout {
                Kirigami.FormData.label: "SD card:"
                QQC2.TextField { id: ns2jb; QQC2.ToolTip.text: text; QQC2.ToolTip.visible: hovered && text.length > 0; QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay; Layout.fillWidth: true; placeholderText: "/run/media/you/SDCARD" }
                QQC2.Button { icon.name: "folder-open"; onClicked: { picker.target = ns2jb; picker.open(); } }
            }
            QQC2.CheckBox {
                id: ns2jbOnSave
                text: "Export on every save"
            }
            QQC2.Label {
                Layout.fillWidth: true
                // a label's implicit width is its unwrapped text: cap it so the dialog keeps its width
                Layout.maximumWidth: Kirigami.Units.gridUnit * 30
                wrapMode: Text.Wrap
                opacity: 0.7
                text: "Copies the mod to <SD>/ns2jb-modloader/contents/0400c3f00006e000/romfs, next to any other Mario Kart World mods there (the resource size table is merged, not replaced). Then load it with ns2jb-modloader: node load.cjs, wait for READY, open the game."
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: "Testing"
            }
            QQC2.TextField {
                id: testCommand
                Kirigami.FormData.label: "Test command:"
                Layout.fillWidth: true
                placeholderText: "eden -g /path/to/your/game"
            }
            QQC2.Label {
                Layout.fillWidth: true
                Layout.maximumWidth: Kirigami.Units.gridUnit * 30
                wrapMode: Text.Wrap
                opacity: 0.7
                text: "File → Save and Test (F5) saves the mod, then runs this shell command, for example an emulator started with your game. Leave it empty to only save."
            }
        }
    }
}
