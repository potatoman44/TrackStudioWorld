# TrackStudioWorld

A course editor for Mario Kart World, inspired by Track Studio for Mario Kart 8 Deluxe. It
opens the game's course files, lets you move, add and delete objects and paths in a 3D view,
and saves your changes as a mod for an emulator or for a Switch 2 with ns2jb-modloader.

![TrackStudioWorld editing the start of Mario Bros. Circuit](docs/screenshots/editor.png)

It is written in Rust. The interface uses Kirigami (KDE Frameworks 6) and the 3D view uses
Qt Quick 3D, bridged with CXX-Qt.

> **Not affiliated.** TrackStudioWorld is an independent project. It is not affiliated with,
> endorsed by or connected to [Track Studio](https://mapstudioproject.github.io/TrackStudioDocs/)
> or the Map Studio Project; the name only says what it was inspired by. It is not affiliated
> with or endorsed by Nintendo either.
>
> It contains no game code or assets: you need your own dump of a game you own, and the
> console keys from your own console.

## What it edits

- **Objects** (`Actors`): item boxes, coins, start positions, audiences, obstacles, areas and
  every other type in the game's object database (4,826 types). Parameters are typed from the
  game's own parameter tables and show their defaults; links between objects can be added and
  removed; any other field can be edited as typed JSON.
- **Paths** (`Rails`), all 16 types: lap paths with their checkpoint gates, CPU and item routes,
  glide paths, guide rails, rail slides, Rainbow Road's gravity paths, citizen, water, parking,
  off-road and pull paths. Points, Bézier handles, checkpoint rectangles and per-point
  parameters are editable, and points can be inserted, deleted and linked.
- **Path data** (`Banc/PathAdditionalData`): per-point width and clearance values.

Any course file opens: races, the routes between courses, knockout rallies, battles, missions,
parades and the free-roam grid cells. Stage models are drawn where the game places them, and
objects with their real models.

On save the editor rebuilds the counts the game reads to decide which systems a course needs
(`CountActor`, `CountRail`, `Collects`), grows `AABB` over moved objects, refuses duplicate
hashes, writes BYML exactly the way Nintendo's tools do (an unedited course saves
byte-identical), and raises the resource size table only when a file outgrew its entry.

## Screenshots

| A whole course, with every path drawn over the stage | A lap path point, its checkpoint gate and its parameters | Any course file opens, searchable by file or course name |
|---|---|---|
| [![A whole course with its paths](docs/screenshots/overview.png)](docs/screenshots/overview.png) | [![Editing a lap path point and its checkpoint gate](docs/screenshots/paths.png)](docs/screenshots/paths.png) | [![Opening a course](docs/screenshots/open-course.png)](docs/screenshots/open-course.png) |

## How it was checked

- **Against every shipped file**: the BYML writer, the summaries, the size table and the
  parameter types reproduce all 1,311 shipped Banc files ([docs/format.md](docs/format.md)).
- **Against the game's code**: the keys the loader reads, the rotation order and what the counts
  control were read from the executable ([docs/game-code.md](docs/game-code.md)).
- **In game**: an emulator test harness boots the game with mods the editor wrote and drives
  into a Grand Prix race; objects added and moved in the editor appear where they were put and
  work ([docs/testing.md](docs/testing.md)).

## Building

You need:

- Rust 1.90 or newer;
- Qt 6 with Qt Quick 3D, Kirigami and the KDE desktop style (KDE Frameworks 6), and the Breeze
  widget style and icons. It is developed with Qt 6.12 and KDE Frameworks 6.31.

On Arch Linux:

```sh
sudo pacman -S --needed rust qt6-base qt6-declarative qt6-quick3d kirigami qqc2-desktop-style breeze breeze-icons
cargo build --release
```

The binary is `target/release/trackstudioworld`. `.cargo/config.toml` makes the build link
with Rust's bundled lld; see the comment there for why.

## Game files

The editor reads the game's extracted RomFS (with `Banc/`, `RSDB/`, `System/`, `Model/` and
`Tex/`), version 1.8.0. Course data, models and textures all come from it: the editor reads the
game's BFRES models and BNTX textures itself, so there is nothing to export first.

Set it in **File → Settings…**, or start with:

```sh
trackstudioworld --romfs /path/to/romfs
```

| Option | Meaning |
|---|---|
| `--romfs DIR` | The game's extracted RomFS |
| `--mods DIR` | Where mods are saved (default `~/MKW-Mods`) |
| `--ns2jb DIR` | A Switch 2 SD card (or a folder to copy onto one): export there on every save |
| `--open FILE` | Open a course at start, e.g. `"Banc/Race_FirstCircuit_A_00[Game03].bcett.byml.zs"` |
| `--select NAME` | After opening, select the first object of that type, or `PATH#N` for point N of the first path with that name |

Settings are saved in `~/.config/trackstudioworld/config.json`. The first start reads the
game's databases and every course, about 7 s, and caches the result in
`~/.cache/trackstudioworld/`.

## Using it

1. **Open a course** with **File → Open Course…** (Ctrl+O); search by file name or by the
   course's Japanese name. Pick a mod under **From:** to keep editing one you saved.
2. **Select** by clicking in the view or the Outliner. Ctrl+Click adds to the selection;
   clicking a group in the Outliner selects every object of that type.
   Drag in empty space to box-select (Ctrl adds, Ctrl+Shift removes), Ctrl+A selects
   everything, H hides the selection and Alt+H shows everything again. The **Editor** chip over
   the view limits what clicks pick to objects, paths, or one path type, like Track Studio's
   editor list.
3. **Edit objects**:
   - move, rotate or scale with the gizmo (1, 2, 3 switch modes), or with the mouse: G, R and T,
     then X, Y or Z to lock an axis, a click to finish and Esc to cancel; or type values in
     Properties. **Snap** sets move and rotation steps; Ctrl inverts snapping for one drag;
   - copy, cut and paste (Ctrl+C, Ctrl+X, Ctrl+V at the view centre, Ctrl+Shift+V in place, also
     between courses), duplicate (Ctrl+D), delete (Del), drop onto the stage surface (End), or
     change an object into another type (**Change Object…**);
   - add the current asset with a double-click in Assets, Q, Alt+Click in the view, or by
     dragging it from Assets into the view; star assets to keep them under **★ Favourites**;
   - select several objects or points to see their centre and size, move them together, and set
     a parameter on all of them at once.
4. **Edit paths** (Paths menu):
   - **New Path…** starts any path type as one point, a line, a curve or a closed loop; with a
     path type picked in the Editor chip, Alt+Click starts one where you click;
   - with a point selected, Alt+Click adds the next point there (from a middle point it starts a
     branch), E extrudes a point and moves it with the mouse, I inserts a point after it;
   - C connects or disconnects two selected points, **Split Path Here** and **Join Paths**
     (Ctrl+J) cut and merge paths, **Reverse Path** turns them around;
   - **Orient Points Along Path** (O) turns points to face along the path, upright on the road,
     as shipped points do; **Smooth Curve**, **Straighten** and **Subdivide** reshape a path, and
     curve handles can be dragged in the view (hold Ctrl to keep both in line);
   - **Change Path Type…** converts paths, **Place Objects Along Path…** lines the current
     asset up along one (a coin line, say), and L links the selection to the next click;
   - tick **Checkpoint** in the Lap section of Properties to make lap points checkpoints. Their
     numbers are kept in order for you: after every edit they count up from 1 along the lap,
     starting again after each start, lap check and goal point, as in the shipped courses.
     Checkpoint gates are drawn with a cross, start, lap check and goal gates in white.
5. **Check** the **Problems** tab: what would stop a save, and anything the shipped courses never
   do (a race without a lap path, lap points facing backwards, a lap path that doesn't loop, the
   wrong number of start positions, lap checkpoints missing or out of order, parameters stored
   with the wrong type). Click a problem to select it.
6. **Save** with Ctrl+S, or **Save and Test** (F5) to also run a command from Settings, such as
   an emulator. Undo and redo (Ctrl+Z, Ctrl+Shift+Z) name what they undo; the editor asks before
   closing or opening another course with unsaved changes, and **File → Open Recent** lists
   recent courses. Right-click the view or the Outliner for the common actions; the Help panel
   lists every key.

The view has a **top view** (Ctrl+T, orthographic), Alt+drag orbiting, and lit, unlit, normals
and wireframe shading.

### New courses and replacing courses

**File → New Course…** (Ctrl+N) starts a course at the place of a shipped course (its stage is
drawn around it), either empty or as a copy of that course's objects and paths. **File → Save
As (Replace Course)…** (Ctrl+Shift+S) saves the open course into the mod as any course file you
pick, with its own PathAdditionalData and `FilePath`, so it replaces that course in game; saving a
new course asks for this first. Each course's place in the world comes from the game's course
table, not the course file, so the scenery that loads is the replaced course's: build near the
course you replace (Save As warns when the course is over a kilometre away).

### Importing models

**File → Import Model…** turns an OBJ, FBX (binary or ASCII) or COLLADA (`.dae`) model into a
new object type in the open mod, listed in Assets under **Custom** and placed like any other
object. The model is built from a shipped object (a static block wall): it keeps that object's
shaders, materials and baked lighting and takes the imported geometry and each material's colour
texture (or colour), up to 6 materials. Files that state their unit (FBX, COLLADA) are converted
to metres, the game's unit; OBJ files are taken as they are, with a scale in the dialog.

An import writes the model and its textures, adds the object's definitions to the
`System/GymlArchive` packs that hold the block wall's (the mod gets whole copies of those packs),
adds a row to `RSDB/Actor`, and adds size table entries for all of them. The mod's imports are
listed in `<mods>/<mod name>/custom_objects.json`, next to `romfs/`. Importing a name again
replaces that object. **Imported objects have no collision yet.**

**File → Model Editor…** (Ctrl+M) opens a window with the mod's custom objects and a preview.
For each object you can change its label, scale, rotation and offset (or put its origin at the
middle of its bottom). For each material you can replace or add its texture, tint it, scale and
shift its UVs, flip V, choose how bright the borrowed baked lighting is, and hide it. **Apply**
rebuilds the object from its source: every import keeps its meshes and images in
`<mods>/<mod name>/custom_src/<name>/`, so edits never pile up. **Remove** takes an object out of
the mod again.

## Installing a mod

The editor writes a LayeredFS mod to `<mods>/<mod name>/romfs/`.

- **Eden and its forks** (PokeDinner, Eglantine): copy the mod folder into
  `~/.local/share/eden/load/0400C3F00006E000/`, or set the mods folder to that path to save
  straight into it. If a mod seems to do nothing, look for
  `LayeredFS patches applied successfully` in the emulator's log, and check `load_directory`
  in its settings.
- **Switch 2 with ns2jb-modloader**: **File → Export for Switch 2** copies the mod into the SD
  card's `ns2jb-modloader/contents/0400c3f00006e000/romfs` and merges its size table with the
  one already there, so other mods keep working.

## Limitations

- Path widths and clearances (`PathAdditionalData`) are not regenerated when route points
  move; edit them by hand if a route changes a lot.
- Object types never placed in a shipped course get only `IsGridify`, as there is no shipped
  placement to copy their other fields from.
- Reversing a path doesn't rewire the links at its ends.
- Imported models have no collision (Havok `.bphsh` shapes are not written yet), share one
  baked-lighting value, and are limited to 6 materials. No camera, minimap or lighting editing;
  the open world is generated from heightmaps.
- Models are drawn with their albedo texture only, at the most detailed level of detail;
  skinned models show in their bind pose.

## Repository

| Path | What it is |
|---|---|
| `core/` (`mkwtrack`) | The library: file formats, the game catalog, editing with undo, saving mods, picking and scene data. No Qt. |
| `app/` (`trackstudioworld`) | The desktop app. All logic is Rust; QML lays out the panels and draws what Rust hands it. |
| `tools/emutest/` (`mkw-emutest`) | The in-game test harness. |
| `docs/` | How the files work, what the game's code says, and how it is all tested. |

See [CONTRIBUTING.md](CONTRIBUTING.md) to build, test and change it.

## Acknowledgements

- Rimea, "Datamining Mario Kart World - How The Game Works & Unused Content" (2026-11-04), for
  the Banc layout, the Game03 naming, lap checkpoints and Rainbow Road's gravity paths.
- The Track Studio documentation (https://mapstudioproject.github.io/TrackStudioDocs/), for the
  feature set this follows.
- PokeDinner, an Eglantine fork (https://git.eden-emu.dev/Franseven/PokeDinner-a-Eglantine-Fork),
  for running the game and applying LayeredFS mods to loose dumps.

## License

[0BSD](LICENSE).

***

AI/SI models are used and encouraged in the development of this project, see [AIPOLICY](AIPOLICY.md).
