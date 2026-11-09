# Contributing

Thanks for helping. A few things keep this project useful and safe to share.

## Ground rules

- **No game files in the repository**: no romfs or exefs files, no extracted models or
  textures, no keys. Tests that need game files read them from paths you give them. The
  editor screenshots in `docs/screenshots/` are the exception; keep personal paths out of
  them.
- **Back format claims with evidence.** When code depends on how the game reads a file, say how
  you know: checked against every shipped file, read from the executable (with the address and
  game version), or seen in game. [docs/format.md](docs/format.md) and
  [docs/game-code.md](docs/game-code.md) collect what is known; add to them.
- **The whole project is Rust.** QML only lays out panels and draws what Rust hands it.

## Layout

| Path | What it is |
|---|---|
| `core/src/byml.rs`, `sarc.rs`, `restbl.rs`, `typedjson.rs`, `io.rs` | File formats |
| `core/src/banc.rs` | Course files: names, the summaries the game reads, checks before saving |
| `core/src/workspace.rs` | Loading courses and saving mods (emulator and ns2jb-modloader) |
| `core/src/catalog.rs` | Object types, parameter tables, path types and stage placements, cached as JSON |
| `core/src/doc.rs` | An open course and every edit, with undo |
| `core/src/tools.rs` | Copy and paste, path drawing and reshaping, placing along paths |
| `core/src/check.rs` | The course checks the Problems panel lists |
| `core/src/bfres.rs`, `bntx.rs`, `model.rs` | The game's models and textures |
| `core/src/bfres_write.rs`, `custom.rs` | Writing models from a shipped template, and importing them into a mod as new object types |
| `core/src/import/` | Reading OBJ, FBX and COLLADA models and their images |
| `core/src/math.rs`, `raycast.rs`, `scene.rs` | Transforms, picking, what the viewport draws |
| `app/src/editor.rs` | The QObject QML talks to (CXX-Qt bridge) |
| `app/src/state.rs` | Editor state behind the bridge, and the JSON the panels show |
| `app/src/prefs.rs` | Recent courses, favourite assets and view settings (`editor.json`) |
| `app/qml/` | The panels and the 3D view |
| `tools/emutest/` | The in-game test harness |

## Checks

Before sending a change:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test` runs the unit tests, which need no game files. With a dump, also run the tests
against the shipped files (absolute paths; cargo runs tests from each crate's folder):

```sh
MKW_ROMFS=/abs/path/romfs cargo test --release -p mkwtrack
```

Changes to how files are written or which fields are set should also pass the in-game tests;
see [docs/testing.md](docs/testing.md).

## Working on the interface

QML is compiled into the binary, so QML changes need a rebuild. For qmlls (the QML language
server), create a `.qmlls.ini` next to the QML that points at the build's module folder (it is
ignored by git, as the path is per machine):

```ini
[General]
buildDir="/abs/path/to/track-editor/target/cxxqt/qml_modules"
no-cmake-calls=true
```

To screenshot the app without a display, run it on Xvfb with
`QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1`; `--open` and `--select` put a course and a
selection on screen at start.
