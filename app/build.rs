use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("TrackStudioWorld").qml_files([
        "qml/ActorGroup.qml",
        "qml/AssetBrowser.qml",
        "qml/AxisIndicator.qml",
        "qml/ChangeObjectDialog.qml",
        "qml/ConsolePanel.qml",
        "qml/DialogFooter.qml",
        "qml/Gizmo.qml",
        "qml/GizmoHandle.qml",
        "qml/HelpPanel.qml",
        "qml/ImportModelDialog.qml",
        "qml/LineModel.qml",
        "qml/Main.qml",
        "qml/MeshPart.qml",
        "qml/NewPathDialog.qml",
        "qml/OpenCourseDialog.qml",
        "qml/Outliner.qml",
        "qml/Panel.qml",
        "qml/ParamRow.qml",
        "qml/PlaceAlongDialog.qml",
        "qml/PrimitiveGeometry.qml",
        "qml/ProblemsPanel.qml",
        "qml/PropertiesPanel.qml",
        "qml/RailLayer.qml",
        "qml/SaveDialog.qml",
        "qml/Section.qml",
        "qml/SettingsDialog.qml",
        "qml/VecRow.qml",
        "qml/ViewMenu.qml",
        "qml/Viewport.qml",
    ]))
    .files(["src/editor.rs"])
    .build();
    println!("cargo:rerun-if-changed=data/kdeglobals");
}
