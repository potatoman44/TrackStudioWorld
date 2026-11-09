pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick3D
import QtQuick3D.Helpers
import org.kde.kirigami as Kirigami
import TrackStudioWorld

// The model editor: the open mod's custom objects, a preview, and how each is built from its
// source model (size, turn, position, and per material its texture, tint, UVs, lighting).
// Changes are kept in a draft until Apply rebuilds the object.
QQC2.ApplicationWindow {
    id: win
    required property Editor editor
    // ask the main window to open its Import Model dialog
    signal importRequested

    title: "Model Editor — TrackStudioWorld"
    width: 1320
    height: 820
    minimumWidth: 980
    minimumHeight: 600
    color: Kirigami.Theme.backgroundColor

    property var objects: []
    property int current: -1
    readonly property var obj: current >= 0 && current < objects.length ? objects[current] : null
    // the settings being edited (a copy of obj.settings) and whether they differ from it
    property var draft: null
    property string draftLabel: ""
    property bool changed: false
    property string error: ""
    property int parts: 0

    function reload(keep) {
        const name = keep && obj ? obj.name : "";
        objects = JSON.parse(editor.customObjects());
        let i = objects.length > 0 ? 0 : -1;
        for (let k = 0; k < objects.length; k++)
            if (objects[k].name === name)
                i = k;
        current = -1;
        current = i;
    }
    onCurrentChanged: {
        error = "";
        if (!obj) {
            draft = null;
            parts = 0;
            return;
        }
        draft = JSON.parse(JSON.stringify(obj.settings));
        draftLabel = obj.label;
        changed = false;
        parts = 0;
        parts = editor.previewModel(obj.model);
        view.frame();
    }
    // set one draft value; `path` is a list of keys
    function set(path, value) {
        let o = draft;
        for (let i = 0; i < path.length - 1; i++)
            o = o[path[i]];
        o[path[path.length - 1]] = value;
        changed = true;
    }
    function apply() {
        if (!obj)
            return;
        error = editor.updateCustom(obj.name, draftLabel, JSON.stringify(draft));
        if (error.length === 0)
            reload(true);
    }

    onVisibleChanged: if (visible) reload(true)
    Connections {
        target: win.editor
        // an import or another change elsewhere
        function onCustomRevisionChanged() {
            if (win.visible && !win.changed)
                win.reload(true);
        }
        function onModNameChanged() {
            if (win.visible)
                win.reload(false);
        }
    }

    FileDialog {
        id: texturePicker
        property int target: 0
        title: "Choose a texture"
        nameFilters: ["Images (*.png *.jpg *.jpeg *.tga *.bmp)"]
        onAccepted: {
            const path = decodeURIComponent(selectedFile.toString()).replace(/^file:\/\//, "").replace(/^\/([A-Za-z]:)/, "$1");
            // the other unapplied changes go first, so the rebuild keeps them
            if (win.changed)
                win.apply();
            win.error = win.editor.setCustomTexture(win.obj.name, target, path);
            if (win.error.length === 0)
                win.reload(true);
        }
    }
    ColorDialog {
        id: tintPicker
        property int target: 0
        title: "Tint"
        onAccepted: {
            win.set(["materials", target, "tint"], [selectedColor.r, selectedColor.g, selectedColor.b, selectedColor.a]);
            materials.model = 0;
            materials.model = win.draft.materials.length;
        }
    }
    Kirigami.PromptDialog {
        id: deleteDialog
        title: "Remove Object"
        subtitle: win.obj ? "Remove \"" + win.obj.label + "\" (" + win.obj.actor + ") from the mod \"" + win.editor.modName + "\"? Objects of this type already placed in courses will no longer load." : ""
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: {
            win.error = win.editor.deleteCustom(win.obj.name);
            win.reload(false);
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // ------------------------------------------------------------ objects
        ColumnLayout {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 15
            Layout.fillHeight: true
            Layout.margins: Kirigami.Units.smallSpacing
            Kirigami.Heading {
                level: 4
                text: "Custom objects in \"" + win.editor.modName + "\""
                Layout.fillWidth: true
                elide: Text.ElideRight
            }
            QQC2.ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                ListView {
                    id: list
                    clip: true
                    model: win.objects
                    currentIndex: win.current
                    delegate: QQC2.ItemDelegate {
                        id: d
                        required property var modelData
                        required property int index
                        width: ListView.view.width
                        highlighted: index === win.current
                        contentItem: ColumnLayout {
                            spacing: 0
                            QQC2.Label { text: d.modelData.label; elide: Text.ElideRight; Layout.fillWidth: true }
                            QQC2.Label { text: d.modelData.actor; opacity: 0.6; font: Kirigami.Theme.smallFont; elide: Text.ElideRight; Layout.fillWidth: true }
                        }
                        onClicked: win.current = index
                    }
                }
            }
            QQC2.Label {
                visible: win.objects.length === 0
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                opacity: 0.7
                text: "No custom objects in this mod yet. Import a model to start."
            }
            RowLayout {
                QQC2.Button {
                    text: "Import…"
                    icon.name: "document-import"
                    onClicked: win.importRequested()
                }
                QQC2.Button {
                    text: "Remove"
                    icon.name: "edit-delete"
                    enabled: win.obj !== null
                    onClicked: deleteDialog.open()
                }
            }
        }
        Kirigami.Separator { Layout.fillHeight: true }

        // ------------------------------------------------------------ preview
        View3D {
            id: view
            Layout.fillWidth: true
            Layout.fillHeight: true
            environment: SceneEnvironment {
                clearColor: Qt.darker(Kirigami.Theme.backgroundColor, 1.6)
                backgroundMode: SceneEnvironment.Color
                antialiasingMode: SceneEnvironment.MSAA
            }
            // frame the object's bounds
            function frame() {
                const b = win.obj ? win.obj.aabb : [[-1, -1, -1], [1, 1, 1]];
                const c = Qt.vector3d((b[0][0] + b[1][0]) / 2, (b[0][1] + b[1][1]) / 2, (b[0][2] + b[1][2]) / 2);
                const r = Math.max(0.5, Math.hypot(b[1][0] - b[0][0], b[1][1] - b[0][1], b[1][2] - b[0][2]) / 2);
                origin.position = c;
                camera.position = Qt.vector3d(0, r * 0.6, r * 2.4);
                camera.lookAt(c);
                camera.clipNear = Math.max(0.01, r / 200);
                camera.clipFar = r * 50;
            }
            Node {
                id: origin
                PerspectiveCamera {
                    id: camera
                    fieldOfView: 40
                }
            }
            DirectionalLight {
                eulerRotation: Qt.vector3d(-40, -30, 0)
                brightness: 1.2
            }
            DirectionalLight {
                eulerRotation: Qt.vector3d(-20, 150, 0)
                brightness: 0.4
            }
            Node {
                // rebuilt objects keep their name: re-create the parts when the model changes
                Repeater3D {
                    model: win.parts
                    delegate: MeshPart {
                        required property int index
                        editor: win.editor
                        rel: win.obj ? win.obj.model : ""
                        part: index
                    }
                }
            }
            OrbitCameraController {
                anchors.fill: parent
                origin: origin
                camera: camera
            }
            QQC2.Label {
                anchors.left: parent.left
                anchors.bottom: parent.bottom
                anchors.margins: Kirigami.Units.smallSpacing
                opacity: 0.7
                text: win.obj ? "Drag to orbit, scroll to zoom. The preview shows each material's colour texture; in game the template's lighting applies." : ""
            }
        }
        Kirigami.Separator { Layout.fillHeight: true }

        // ------------------------------------------------------------ settings
        QQC2.ScrollView {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 24
            Layout.fillHeight: true
            contentWidth: availableWidth
            ColumnLayout {
                width: parent.width
                spacing: Kirigami.Units.smallSpacing
                enabled: win.draft !== null && win.obj !== null && win.obj.editable
                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    visible: win.obj !== null && !win.obj.editable
                    type: Kirigami.MessageType.Information
                    text: "This object was imported before the model editor existed, so its source wasn't kept. Import it again to edit it."
                }
                Kirigami.InlineMessage {
                    Layout.fillWidth: true
                    visible: win.error.length > 0
                    type: Kirigami.MessageType.Error
                    text: win.error
                }
                Kirigami.Heading { level: 4; text: "Object" }
                RowLayout {
                    QQC2.Label { text: "Label"; Layout.preferredWidth: Kirigami.Units.gridUnit * 5.5 }
                    QQC2.TextField {
                        Layout.fillWidth: true
                        text: win.draftLabel
                        onTextEdited: {
                            win.draftLabel = text;
                            win.changed = true;
                        }
                    }
                }
                VecRow {
                    label: "Scale"
                    count: 1
                    values: win.draft ? [win.draft.scale] : [1]
                    onCommitted: (v) => win.set(["scale"], v[0])
                }
                VecRow {
                    label: "Rotate °"
                    decimals: 1
                    values: win.draft ? win.draft.rotate : [0, 0, 0]
                    onCommitted: (v) => win.set(["rotate"], v)
                }
                VecRow {
                    label: "Offset"
                    values: win.draft ? win.draft.offset : [0, 0, 0]
                    onCommitted: (v) => win.set(["offset"], v)
                }
                QQC2.CheckBox {
                    text: "Origin at the middle of the bottom"
                    checked: win.draft ? win.draft.ground : false
                    onToggled: win.set(["ground"], checked)
                }

                Kirigami.Heading { level: 4; text: "Materials" }
                Repeater {
                    id: materials
                    model: win.draft ? win.draft.materials.length : 0
                    delegate: Kirigami.AbstractCard {
                        id: card
                        required property int index
                        readonly property var m: win.draft.materials[index]
                        readonly property var info: win.obj && win.obj.materials[index] ? win.obj.materials[index] : ({})
                        Layout.fillWidth: true
                        contentItem: ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing
                            RowLayout {
                                Rectangle {
                                    Layout.preferredWidth: Kirigami.Units.gridUnit * 4
                                    Layout.preferredHeight: Kirigami.Units.gridUnit * 4
                                    color: Qt.rgba(card.m.tint[0], card.m.tint[1], card.m.tint[2], 1)
                                    border.color: Kirigami.Theme.disabledTextColor
                                    Image {
                                        anchors.fill: parent
                                        anchors.margins: 1
                                        source: card.info.image || ""
                                        fillMode: Image.PreserveAspectCrop
                                        cache: false
                                        asynchronous: true
                                    }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    QQC2.Label { text: (card.index + 1) + ". " + (card.m.name || "material"); font.bold: true; elide: Text.ElideRight; Layout.fillWidth: true }
                                    RowLayout {
                                        QQC2.Button {
                                            text: card.info.image ? "Replace Texture…" : "Add Texture…"
                                            icon.name: "insert-image"
                                            onClicked: {
                                                texturePicker.target = card.index;
                                                texturePicker.open();
                                            }
                                        }
                                        QQC2.Button {
                                            text: "Tint…"
                                            icon.name: "color-picker"
                                            onClicked: {
                                                tintPicker.target = card.index;
                                                tintPicker.selectedColor = Qt.rgba(card.m.tint[0], card.m.tint[1], card.m.tint[2], card.m.tint[3]);
                                                tintPicker.open();
                                            }
                                        }
                                    }
                                }
                            }
                            VecRow {
                                label: "UV scale"
                                count: 2
                                values: card.m.uv_scale
                                onCommitted: (v) => win.set(["materials", card.index, "uv_scale"], v)
                            }
                            VecRow {
                                label: "UV offset"
                                count: 2
                                values: card.m.uv_offset
                                onCommitted: (v) => win.set(["materials", card.index, "uv_offset"], v)
                            }
                            RowLayout {
                                QQC2.Label { text: "Lighting"; Layout.preferredWidth: Kirigami.Units.gridUnit * 5.5 }
                                QQC2.Label { text: "darker"; opacity: 0.6 }
                                QQC2.Slider {
                                    Layout.fillWidth: true
                                    from: 0
                                    to: 1
                                    value: card.m.light
                                    onMoved: win.set(["materials", card.index, "light"], value)
                                }
                                QQC2.Label { text: "brighter"; opacity: 0.6 }
                            }
                            RowLayout {
                                QQC2.CheckBox {
                                    text: "Flip V"
                                    checked: card.m.flip_v
                                    onToggled: win.set(["materials", card.index, "flip_v"], checked)
                                }
                                QQC2.CheckBox {
                                    text: "Visible"
                                    checked: card.m.visible
                                    onToggled: win.set(["materials", card.index, "visible"], checked)
                                }
                            }
                        }
                    }
                }
                RowLayout {
                    Layout.topMargin: Kirigami.Units.largeSpacing
                    Item { Layout.fillWidth: true }
                    QQC2.Button {
                        text: "Revert"
                        icon.name: "edit-undo"
                        enabled: win.changed
                        onClicked: {
                            const c = win.current;
                            win.current = -1;
                            win.current = c;
                        }
                    }
                    QQC2.Button {
                        text: "Apply"
                        icon.name: "dialog-ok-apply"
                        highlighted: true
                        enabled: win.changed && !win.editor.busy
                        onClicked: win.apply()
                    }
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    opacity: 0.7
                    text: "Apply rebuilds the object in the mod from its source model with these settings; save the course afterwards as usual. Lighting picks how bright the borrowed baked lighting is. A game unit is a metre."
                }
            }
        }
    }
}
