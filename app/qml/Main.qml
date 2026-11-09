pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import TrackStudioWorld

Kirigami.ApplicationWindow {
    id: root

    width: 1680
    height: 1000
    minimumWidth: 1180
    minimumHeight: 720
    title: (editor.dirty ? "● " : "") + (editor.courseTitle.length > 0 ? editor.courseTitle + " — " : "") + "TrackStudioWorld"
    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None

    property string currentAsset: "CmnObj_ItemBox000_A_00"
    property string cursorText: ""
    property var recent: []
    // set once the user chose to quit without saving
    property bool quitting: false

    Editor {
        id: editor
        Component.onCompleted: start()
        onReadyChanged: {
            if (!ready)
                return;
            root.recent = JSON.parse(recentCourses());
            helpPanel.collapsed = JSON.parse(pref("helpCollapsed")) !== false;
            // asset previews are cached: this only renders the ones still missing, in the background
            buildThumbnails();
            const first = startupCourse();
            if (first.length > 0)
                openCourse(first, "");
            else
                openDialog.open();
        }
        onCourseTitleChanged: root.recent = JSON.parse(recentCourses())
    }

    // Run `then` now, or after asking what to do with unsaved changes.
    function confirmDiscard(then) {
        if (!editor.dirty) {
            then();
            return;
        }
        unsavedDialog.then = then;
        unsavedDialog.open();
    }
    onClosing: (close) => {
        if (editor.dirty && !root.quitting) {
            close.accepted = false;
            confirmDiscard(() => {
                root.quitting = true;
                root.close();
            });
        }
    }

    // ------------------------------------------------------------ actions
    // what is selected, as the Properties panel shows it
    readonly property var sel: props.s
    readonly property bool hasCourse: editor.courseTitle.length > 0
    readonly property bool hasSel: ["actor", "point", "rail", "multi"].includes(sel.kind)
    readonly property bool pathSel: sel.kind === "point" || sel.kind === "rail" || (sel.kind === "multi" && (sel.rails || 0) > 0)

    Kirigami.Action {
        id: openAction
        text: "Open Course…"
        icon.name: "document-open"
        shortcut: "Ctrl+O"
        enabled: editor.ready
        onTriggered: openDialog.open()
    }
    Kirigami.Action {
        id: newCourseAction
        text: "New Course…"
        icon.name: "document-new"
        shortcut: "Ctrl+N"
        enabled: editor.ready
        onTriggered: newCourseDialog.open()
    }
    Kirigami.Action {
        id: saveAction
        text: "Save Mod"
        icon.name: "document-save"
        shortcut: "Ctrl+S"
        enabled: root.hasCourse
        onTriggered: root.showSave(JSON.parse(editor.save()))
    }
    Kirigami.Action {
        id: saveAsAction
        text: "Save As (Replace Course)…"
        icon.name: "document-save-as"
        shortcut: "Ctrl+Shift+S"
        enabled: root.hasCourse
        onTriggered: saveAsDialog.open()
    }
    // a new course has no file yet: saving it first asks which course it replaces
    function showSave(r) {
        if (r.needTarget === true) {
            saveAsDialog.open();
            return;
        }
        saveDialog.saveResult = r;
        saveDialog.open();
    }
    Kirigami.Action {
        id: testAction
        text: "Save and Test"
        icon.name: "media-playback-start"
        shortcut: "F5"
        enabled: root.hasCourse
        onTriggered: {
            const r = JSON.parse(editor.runTest());
            if (r.ok !== true)
                root.showSave(r);
        }
    }
    Kirigami.Action {
        id: undoAction
        text: "Undo" + (editor.undoLabel.length > 0 ? " " + editor.undoLabel : "")
        icon.name: "edit-undo"
        shortcut: "Ctrl+Z"
        enabled: editor.canUndo
        onTriggered: editor.undo()
    }
    Kirigami.Action {
        id: redoAction
        text: "Redo" + (editor.redoLabel.length > 0 ? " " + editor.redoLabel : "")
        icon.name: "edit-redo"
        shortcut: "Ctrl+Shift+Z"
        enabled: editor.canRedo
        onTriggered: editor.redo()
    }
    Kirigami.Action {
        id: cutAction
        text: "Cut"
        icon.name: "edit-cut"
        shortcut: "Ctrl+X"
        enabled: root.hasSel
        onTriggered: editor.copySelection(true)
    }
    Kirigami.Action {
        id: copyAction
        text: "Copy"
        icon.name: "edit-copy"
        shortcut: "Ctrl+C"
        enabled: root.hasSel
        onTriggered: editor.copySelection(false)
    }
    Kirigami.Action {
        id: pasteAction
        text: "Paste"
        icon.name: "edit-paste"
        shortcut: "Ctrl+V"
        enabled: root.hasCourse && editor.hasClipboard
        onTriggered: viewport.pasteAtCenter(false)
    }
    Kirigami.Action {
        id: pasteInPlaceAction
        text: "Paste in Place"
        icon.name: "edit-paste-in-place"
        shortcut: "Ctrl+Shift+V"
        enabled: root.hasCourse && editor.hasClipboard
        onTriggered: viewport.pasteAtCenter(true)
    }
    Kirigami.Action {
        id: duplicateAction
        text: "Duplicate"
        icon.name: "edit-duplicate"
        shortcut: "Ctrl+D"
        enabled: root.hasSel
        onTriggered: viewport.duplicate()
    }
    Kirigami.Action {
        id: deleteAction
        text: "Delete"
        icon.name: "edit-delete"
        shortcut: "Del"
        enabled: root.hasSel
        onTriggered: editor.deleteSelected()
    }
    Kirigami.Action {
        id: dropAction
        text: "Drop to Ground"
        icon.name: "go-bottom"
        shortcut: "End"
        enabled: editor.groundReady && root.hasSel
        onTriggered: editor.dropToGround()
    }
    Kirigami.Action {
        id: addAction
        text: "Add " + viewport.shortName(root.currentAsset)
        icon.name: "list-add"
        shortcut: "Q"
        enabled: root.hasCourse
        onTriggered: viewport.addAtCenter(root.currentAsset)
    }
    Kirigami.Action {
        id: changeAction
        text: "Change Object…"
        icon.name: "exchange-positions"
        enabled: root.sel.kind === "actor" || root.sel.kind === "multi" && (root.sel.objects || 0) > 0
        onTriggered: changeDialog.open()
    }
    Kirigami.Action {
        id: selectAllAction
        text: "Select All"
        icon.name: "edit-select-all"
        shortcut: "Ctrl+A"
        enabled: root.hasCourse
        onTriggered: editor.selectAll()
    }
    Kirigami.Action {
        id: selectNoneAction
        text: "Select None"
        icon.name: "edit-select-none"
        enabled: root.hasSel
        onTriggered: editor.clearSelection()
    }
    Kirigami.Action {
        id: hideAction
        text: "Hide"
        icon.name: "view-hidden"
        shortcut: "H"
        enabled: root.hasSel
        onTriggered: editor.hideSelected()
    }
    Kirigami.Action {
        id: unhideAction
        text: "Show All"
        icon.name: "view-visible"
        shortcut: "Alt+H"
        enabled: root.hasCourse && (viewport.viewInfo.hidden || 0) > 0
        onTriggered: editor.unhideAll()
    }
    Kirigami.Action {
        id: newPathAction
        text: "New Path…"
        icon.name: "draw-polyline"
        enabled: root.hasCourse
        onTriggered: {
            newPathDialog.mode = "new";
            newPathDialog.open();
        }
    }
    Kirigami.Action {
        id: extrudeAction
        text: "Extrude Point"
        icon.name: "node-add"
        shortcut: "E"
        enabled: root.sel.kind === "point"
        onTriggered: viewport.extrude()
    }
    Kirigami.Action {
        id: insertPointAction
        text: "Insert Point After"
        icon.name: "node-add"
        shortcut: "I"
        enabled: root.sel.kind === "point"
        onTriggered: editor.insertPoint()
    }
    Kirigami.Action {
        id: connectAction
        text: "Connect / Disconnect Points"
        icon.name: "node-join"
        shortcut: "C"
        enabled: root.sel.kind === "multi" && root.sel.points === 2 && root.sel.count === 2
        onTriggered: editor.connectPoints()
    }
    Kirigami.Action {
        id: splitAction
        text: "Split Path Here"
        icon.name: "node-segment-split"
        enabled: root.sel.kind === "point" && (root.sel.closed === true || root.sel.index + 1 < root.sel.count)
        onTriggered: editor.pathTool("split")
    }
    Kirigami.Action {
        id: joinAction
        text: "Join Paths"
        icon.name: "node-join-segment"
        shortcut: "Ctrl+J"
        enabled: root.sel.kind === "multi" && root.sel.rails === 2
        onTriggered: editor.pathTool("join")
    }
    Kirigami.Action {
        id: orientAction
        text: "Orient Points Along Path"
        icon.name: "transform-rotate"
        shortcut: "O"
        enabled: root.pathSel
        onTriggered: editor.pathTool("orient")
    }
    Kirigami.Action {
        id: smoothAction
        text: "Smooth Curve"
        icon.name: "draw-bezier-curves"
        enabled: root.pathSel
        onTriggered: editor.pathTool("smooth")
    }
    Kirigami.Action {
        id: straightenAction
        text: "Straighten (Remove Handles)"
        icon.name: "draw-line"
        enabled: root.pathSel
        onTriggered: editor.pathTool("straighten")
    }
    Kirigami.Action {
        id: subdivideAction
        text: "Subdivide"
        icon.name: "node-add"
        enabled: root.pathSel
        onTriggered: editor.pathTool("subdivide")
    }
    Kirigami.Action {
        id: curveAction
        text: root.sel.control0 ? "Remove Curve Handles" : "Add Curve Handles"
        icon.name: "draw-bezier-curves"
        enabled: root.sel.kind === "point"
        onTriggered: editor.toggleCurve()
    }
    Kirigami.Action {
        id: reverseAction
        text: "Reverse Path"
        icon.name: "media-playlist-repeat"
        enabled: root.pathSel
        onTriggered: editor.pathTool("reverse")
    }
    Kirigami.Action {
        id: selectPathAction
        text: "Select Whole Path"
        icon.name: "draw-polyline"
        enabled: root.sel.kind === "point" || root.sel.kind === "multi" && (root.sel.points || 0) > 0
        onTriggered: editor.pathTool("select-path")
    }
    Kirigami.Action {
        id: selectPointsAction
        text: "Select Its Points"
        icon.name: "edit-select-all"
        enabled: root.pathSel
        onTriggered: editor.pathTool("select-points")
    }
    Kirigami.Action {
        id: changePathTypeAction
        text: "Change Path Type…"
        icon.name: "exchange-positions"
        enabled: root.pathSel
        onTriggered: {
            newPathDialog.mode = "change";
            newPathDialog.open();
        }
    }
    Kirigami.Action {
        id: placeAlongAction
        text: "Place Objects Along Path…"
        icon.name: "distribute-horizontal"
        enabled: root.sel.kind === "point" || root.sel.kind === "rail"
        onTriggered: {
            placeDialog.asset = root.currentAsset;
            placeDialog.open();
        }
    }
    Kirigami.Action {
        id: linkAction
        text: "Link To…"
        icon.name: "link"
        shortcut: "L"
        enabled: root.sel.kind === "actor" || root.sel.kind === "point"
        onTriggered: editor.startLink("BasicSignal")
    }
    Kirigami.Action {
        id: focusAction
        text: "Focus Selection"
        icon.name: "crosshairs"
        shortcut: "F"
        enabled: root.hasSel
        onTriggered: viewport.focusSelection()
    }
    Kirigami.Action {
        id: frameAction
        text: "Frame Course"
        icon.name: "zoom-fit-best"
        shortcut: "Home"
        enabled: root.hasCourse
        onTriggered: viewport.frameAll()
    }
    Kirigami.Action {
        id: topViewAction
        text: "Top View"
        icon.name: "view-split-top-bottom"
        shortcut: "Ctrl+T"
        checkable: true
        checked: viewport.topView
        enabled: root.hasCourse
        onTriggered: viewport.setTopView(!viewport.topView)
    }
    Kirigami.Action {
        id: importAction
        text: "Import Model…"
        icon.name: "document-import"
        enabled: editor.ready && !editor.busy
        onTriggered: importDialog.open()
    }
    Kirigami.Action {
        id: modelEditorAction
        text: "Model Editor…"
        icon.name: "draw-cuboid"
        shortcut: "Ctrl+M"
        enabled: editor.ready
        onTriggered: {
            modelEditor.show();
            modelEditor.raise();
            modelEditor.requestActivate();
        }
    }
    Kirigami.Action {
        id: settingsAction
        text: "Settings…"
        icon.name: "configure"
        onTriggered: settingsDialog.open()
    }

    Shortcut {
        sequence: "Ctrl+Y"
        onActivated: editor.redo()
    }

    // ------------------------------------------------------------ menus

    menuBar: QQC2.MenuBar {
        QQC2.Menu {
            title: "&File"
            QQC2.MenuItem { action: newCourseAction }
            QQC2.MenuItem { action: openAction }
            QQC2.Menu {
                id: recentMenu
                title: "Open Recent"
                enabled: root.recent.length > 0
                Instantiator {
                    model: root.recent
                    delegate: QQC2.MenuItem {
                        required property var modelData
                        text: modelData.name + (modelData.mod.length > 0 ? "  [" + modelData.mod + "]" : "")
                        onTriggered: root.confirmDiscard(() => editor.openCourse(modelData.file, modelData.mod))
                    }
                    onObjectAdded: (i, o) => recentMenu.insertItem(i, o)
                    onObjectRemoved: (i, o) => recentMenu.removeItem(o)
                }
            }
            QQC2.MenuItem { action: saveAction }
            QQC2.MenuItem { action: saveAsAction }
            QQC2.MenuItem { action: testAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { action: importAction }
            QQC2.MenuItem { action: modelEditorAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem {
                text: "Open Mod Folder"
                icon.name: "folder-open"
                onTriggered: editor.openFolder("mods")
            }
            QQC2.MenuItem {
                text: "Open Emulator Mod Folder"
                icon.name: "folder-games"
                onTriggered: editor.openFolder("emulator")
            }
            QQC2.MenuItem {
                text: "Export for Switch 2 (ns2jb-modloader)"
                icon.name: "media-flash-sd-mmc"
                enabled: root.hasCourse && editor.ns2jbPath.length > 0
                onTriggered: {
                    const r = JSON.parse(editor.exportNs2jb());
                    exportDialog.title = r.ok ? "Exported for ns2jb-modloader" : "Export failed";
                    exportDialog.subtitle = r.ok
                        ? r.files.length + " file(s) copied to " + r.dir + "." + (editor.dirty ? "\n\nUnsaved changes are not included: save first, then export again." : "") + "\n\nOn the PC, run node load.cjs from ns2jb-modloader's ModLoader folder, wait for READY, then open the game."
                        : r.errors.join("\n");
                    exportDialog.open();
                }
            }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { action: settingsAction }
            QQC2.MenuItem {
                text: "Quit"
                icon.name: "application-exit"
                onTriggered: root.close()
            }
        }
        QQC2.Menu {
            title: "&Edit"
            QQC2.MenuItem { action: undoAction }
            QQC2.MenuItem { action: redoAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { action: cutAction }
            QQC2.MenuItem { action: copyAction }
            QQC2.MenuItem { action: pasteAction }
            QQC2.MenuItem { action: pasteInPlaceAction }
            QQC2.MenuItem { action: duplicateAction }
            QQC2.MenuItem { action: deleteAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { action: addAction }
            QQC2.MenuItem { action: changeAction }
            QQC2.MenuItem { action: dropAction }
            QQC2.MenuItem { action: linkAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { action: selectAllAction }
            QQC2.MenuItem { action: selectNoneAction }
            QQC2.MenuItem { action: hideAction }
            QQC2.MenuItem { action: unhideAction }
        }
        QQC2.Menu {
            title: "&Paths"
            QQC2.MenuItem { action: newPathAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { action: extrudeAction }
            QQC2.MenuItem { action: insertPointAction }
            QQC2.MenuItem { action: subdivideAction }
            QQC2.MenuItem { action: connectAction }
            QQC2.MenuItem { action: splitAction }
            QQC2.MenuItem { action: joinAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { action: orientAction }
            QQC2.MenuItem { action: curveAction }
            QQC2.MenuItem { action: smoothAction }
            QQC2.MenuItem { action: straightenAction }
            QQC2.MenuItem { action: reverseAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem { action: selectPathAction }
            QQC2.MenuItem { action: selectPointsAction }
            QQC2.MenuItem { action: changePathTypeAction }
            QQC2.MenuItem { action: placeAlongAction }
        }
        QQC2.Menu {
            title: "&View"
            QQC2.MenuItem { action: focusAction }
            QQC2.MenuItem { action: frameAction }
            QQC2.MenuItem { action: topViewAction }
            QQC2.MenuSeparator {}
            QQC2.MenuItem {
                text: "Problems"
                icon.name: "data-warning"
                onTriggered: bottomPanel.currentTab = 1
            }
            QQC2.MenuItem {
                text: "Build Asset Thumbnails"
                icon.name: "view-preview"
                onTriggered: editor.buildThumbnails()
            }
        }
        QQC2.Menu {
            title: "&Help"
            QQC2.MenuItem {
                text: "Controls"
                icon.name: "help-keybord-shortcuts"
                onTriggered: helpDialog.open()
            }
            QQC2.MenuItem {
                text: "About TrackStudioWorld"
                icon.name: "help-about"
                onTriggered: aboutDialog.open()
            }
        }
    }

    // the actions the viewport's context menu and the Properties panel offer
    readonly property var actionMap: ({
        focus: focusAction, cut: cutAction, copy: copyAction, duplicate: duplicateAction, del: deleteAction, drop: dropAction,
        hide: hideAction, unhide: unhideAction, change: changeAction, link: linkAction, extrude: extrudeAction, insert: insertPointAction,
        connect: connectAction, split: splitAction, join: joinAction, orient: orientAction, smooth: smoothAction, straighten: straightenAction,
        subdivide: subdivideAction, reverse: reverseAction, selectPath: selectPathAction, placeAlong: placeAlongAction,
        changePathType: changePathTypeAction, selectAll: selectAllAction
    })

    // ------------------------------------------------------------ layout

    pageStack.initialPage: Kirigami.Page {
        padding: 0
        globalToolBarStyle: Kirigami.ApplicationHeaderStyle.None

        QQC2.SplitView {
            anchors.fill: parent
            orientation: Qt.Horizontal

            QQC2.SplitView {
                orientation: Qt.Vertical
                QQC2.SplitView.preferredWidth: Kirigami.Units.gridUnit * 18.5
                QQC2.SplitView.minimumWidth: Kirigami.Units.gridUnit * 13

                Panel {
                    title: "Outliner"
                    iconName: "view-list-tree"
                    QQC2.SplitView.fillHeight: true
                    Outliner {
                        anchors.fill: parent
                        editor: editor
                        onOpenRequested: openDialog.open()
                        onFocusRequested: viewport.focusSelection()
                    }
                }
                Panel {
                    id: helpPanel
                    title: "Help"
                    iconName: "help-contents"
                    // folded by default, so the outliner gets the height; Help > Controls has the same
                    collapsible: true
                    collapsed: true
                    property real openHeight: Kirigami.Units.gridUnit * 14.5
                    QQC2.SplitView.preferredHeight: headerHeight
                    QQC2.SplitView.minimumHeight: headerHeight
                    QQC2.SplitView.maximumHeight: collapsed ? headerHeight : Number.POSITIVE_INFINITY
                    onCollapsedChanged: {
                        if (collapsed)
                            openHeight = Math.max(height, Kirigami.Units.gridUnit * 8);
                        QQC2.SplitView.preferredHeight = collapsed ? headerHeight : openHeight;
                        if (editor.ready)
                            editor.setPref("helpCollapsed", JSON.stringify(collapsed));
                    }
                    HelpPanel {
                        anchors.fill: parent
                    }
                }
            }

            QQC2.SplitView {
                orientation: Qt.Vertical
                QQC2.SplitView.fillWidth: true

                Panel {
                    title: "Viewport"
                    iconName: "globe"
                    QQC2.SplitView.fillHeight: true
                    Viewport {
                        id: viewport
                        anchors.fill: parent
                        editor: editor
                        currentAsset: root.currentAsset
                        selection: root.sel
                        actions: root.actionMap
                        onCursorChanged: (text) => root.cursorText = text
                        onRequestNewPath: newPathAction.trigger()
                        onOpenRequested: openDialog.open()
                    }
                }
                Panel {
                    id: bottomPanel
                    QQC2.SplitView.preferredHeight: Kirigami.Units.gridUnit * 14
                    tabs: ["Assets", "Problems" + (editor.errorCount + editor.warningCount > 0 ? " (" + (editor.errorCount + editor.warningCount) + ")" : ""), "Console"]
                    iconName: "folder-images"
                    StackLayout {
                        anchors.fill: parent
                        currentIndex: bottomPanel.currentTab
                        AssetBrowser {
                            editor: editor
                            currentAsset: root.currentAsset
                            onAssetChosen: (name) => root.currentAsset = name
                            onAssetActivated: (name) => {
                                root.currentAsset = name;
                                viewport.addAtCenter(name);
                            }
                            onChangeRequested: (name) => editor.changeObject(name)
                        }
                        ProblemsPanel {
                            editor: editor
                            onFocusRequested: viewport.focusSelection()
                        }
                        ConsolePanel {
                            editor: editor
                        }
                    }
                }
            }

            Panel {
                title: "Properties"
                iconName: "document-properties"
                QQC2.SplitView.preferredWidth: Kirigami.Units.gridUnit * 24.5
                QQC2.SplitView.minimumWidth: Kirigami.Units.gridUnit * 18
                PropertiesPanel {
                    id: props
                    anchors.fill: parent
                    editor: editor
                    actions: root.actionMap
                    onFocusRequested: viewport.focusSelection()
                    onChangeObjectRequested: changeDialog.open()
                    onDuplicateRequested: viewport.duplicate()
                }
            }
        }
    }

    footer: QQC2.ToolBar {
        position: QQC2.ToolBar.Footer
        RowLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing
            Kirigami.Icon {
                source: editor.linkPicking ? "link" : (editor.busy ? "view-refresh" : "dialog-information")
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                color: editor.linkPicking ? Kirigami.Theme.neutralTextColor : Kirigami.Theme.textColor
            }
            QQC2.Label {
                Layout.fillWidth: true
                text: editor.status
                elide: Text.ElideRight
                color: editor.linkPicking ? Kirigami.Theme.neutralTextColor : Kirigami.Theme.textColor
                QQC2.ToolTip.text: editor.status
                QQC2.ToolTip.visible: statusHover.hovered && truncated
                HoverHandler { id: statusHover }
            }
            QQC2.ProgressBar {
                visible: editor.modelsWanted > 0 && editor.modelsLoaded < editor.modelsWanted
                from: 0
                to: Math.max(1, editor.modelsWanted)
                value: editor.modelsLoaded
                Layout.preferredWidth: Kirigami.Units.gridUnit * 8
            }
            QQC2.Label {
                visible: editor.modelsWanted > 0 && editor.modelsLoaded < editor.modelsWanted
                text: "models " + editor.modelsLoaded + "/" + editor.modelsWanted
                color: Kirigami.Theme.disabledTextColor
            }
            // problems: a click shows the list
            QQC2.ToolButton {
                visible: root.hasCourse
                icon.name: editor.errorCount > 0 ? "data-error" : editor.warningCount > 0 ? "data-warning" : "checkmark"
                text: editor.errorCount + editor.warningCount > 0 ? (editor.errorCount > 0 ? editor.errorCount + " error(s) " : "") + (editor.warningCount > 0 ? editor.warningCount + " warning(s)" : "") : "No problems"
                display: QQC2.AbstractButton.TextBesideIcon
                onClicked: bottomPanel.currentTab = 1
                QQC2.ToolTip.text: "Course checks (Problems tab)"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
            QQC2.ToolSeparator { visible: root.hasCourse }
            QQC2.Label {
                visible: editor.selectionCount > 0
                text: editor.selectionCount + " selected"
                color: Kirigami.Theme.disabledTextColor
            }
            QQC2.ToolSeparator { visible: editor.selectionCount > 0 }
            // ground position under the mouse
            QQC2.Label {
                text: "Cursor"
                color: Kirigami.Theme.disabledTextColor
            }
            QQC2.Label {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 10
                text: root.cursorText.length > 0 ? root.cursorText : "–"
                font.features: { "tnum": 1 }
                elide: Text.ElideLeft
                QQC2.ToolTip.text: "Stage surface under the mouse (X, Y, Z)"
                QQC2.ToolTip.visible: cursorHover.hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                HoverHandler { id: cursorHover }
            }
            QQC2.ToolSeparator {}
            QQC2.Label {
                text: "Mod"
                color: Kirigami.Theme.disabledTextColor
            }
            QQC2.TextField {
                text: editor.modName
                Layout.preferredWidth: Kirigami.Units.gridUnit * 9
                onEditingFinished: editor.modName = text
                QQC2.ToolTip.text: "Mods are saved to <mods folder>/<this name>/romfs"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
            QQC2.ToolButton {
                action: saveAction
                display: QQC2.AbstractButton.TextBesideIcon
                highlighted: editor.dirty
            }
        }
    }

    // ------------------------------------------------------------ dialogs

    OpenCourseDialog {
        id: openDialog
        editor: editor
        guard: root.confirmDiscard
    }
    NewCourseDialog {
        id: newCourseDialog
        editor: editor
        guard: root.confirmDiscard
    }
    SaveAsDialog {
        id: saveAsDialog
        editor: editor
        onSaved: (r) => root.showSave(r)
    }
    SaveDialog {
        id: saveDialog
        editor: editor
    }
    SettingsDialog {
        id: settingsDialog
        editor: editor
    }
    NewPathDialog {
        id: newPathDialog
        editor: editor
        onChosen: (name, shape) => viewport.newPath(name, shape)
    }
    ChangeObjectDialog {
        id: changeDialog
        editor: editor
        target: root.sel.kind === "actor" ? root.sel.name : root.sel.kind === "multi" ? (root.sel.objects || 0) + " objects" : ""
    }
    PlaceAlongDialog {
        id: placeDialog
        editor: editor
    }
    ModelEditor {
        id: modelEditor
        editor: editor
        visible: false
        onImportRequested: importDialog.open()
    }
    ImportModelDialog {
        id: importDialog
        editor: editor
        // the new object is the one Q, double-click and Alt+Click add next
        onImported: (actor) => root.currentAsset = actor
    }
    Kirigami.Dialog {
        id: helpDialog
        title: "Controls"
        preferredWidth: Kirigami.Units.gridUnit * 30
        padding: Kirigami.Units.largeSpacing
        HelpPanel {
            implicitHeight: Kirigami.Units.gridUnit * 30
        }
    }
    Kirigami.PromptDialog {
        id: unsavedDialog
        property var then: null
        title: "Unsaved changes"
        subtitle: "The course has changes that are not saved to the mod \"" + editor.modName + "\"."
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: "Save"
                icon.name: "document-save"
                onTriggered: {
                    unsavedDialog.close();
                    const r = JSON.parse(editor.save());
                    if (r.ok === true) {
                        if (unsavedDialog.then)
                            unsavedDialog.then();
                    } else {
                        root.showSave(r);
                    }
                }
            },
            Kirigami.Action {
                text: "Discard"
                icon.name: "edit-delete"
                onTriggered: {
                    unsavedDialog.close();
                    if (unsavedDialog.then)
                        unsavedDialog.then();
                }
            },
            Kirigami.Action {
                text: "Cancel"
                icon.name: "dialog-cancel"
                onTriggered: unsavedDialog.close()
            }
        ]
    }
    Kirigami.PromptDialog {
        id: exportDialog
        standardButtons: Kirigami.Dialog.Ok
    }
    Kirigami.PromptDialog {
        id: aboutDialog
        title: "TrackStudioWorld"
        subtitle: "A course editor for Mario Kart World, inspired by Track Studio for Mario Kart 8 Deluxe.\n\n"
            + "TrackStudioWorld is an independent project. It is not affiliated with or endorsed by Track Studio, "
            + "the Map Studio Project or Nintendo.\n\n"
            + "Version " + Qt.application.version + ". Released under the BSD Zero Clause License."
        standardButtons: Kirigami.Dialog.Ok
    }
}
