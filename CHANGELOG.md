# Changelog

## Unreleased

- **Import Model** (File menu): OBJ, FBX and COLLADA models become new object types in the
  open mod, listed in Assets under Custom. The model is rebuilt from a shipped one (its shaders,
  materials and baked lighting), with BNTX textures, object definitions in the GymlArchive
  packs, an `RSDB/Actor` row and size table entries written for it. No collision yet.
- `mkwtrack`: BNTX texture writing (Switch 2 layout), SARC writing (byte-identical to the shipped
  packs), BFRES rebuilding from a template, and OBJ/FBX/COLLADA readers.
- Models and textures are read from the open mod first, so a mod's own files are drawn.
- Adding an object of a type the course didn't have loads its model straight away.
- Fix: folders picked in Settings on Windows were saved as `C:/C:/...`, so the game files were
  not found.

## 0.1.0 (unreleased)

First public release.

- Course editor for every Banc course file: objects, all 16 path types and path data, with a
  3D view of the stage and object models (read from the RomFS, no export needed), an outliner,
  typed parameters, links and undo.
- Track Studio-style editing: box selection, pick filters per path type, G/R/T transforms
  with axis locks, snapping, copy and paste between courses, hiding, a top view, shading modes,
  orbiting, a context menu, drag and drop from Assets, favourite assets and editing a parameter
  on many items at once.
- Path tools: draw with Alt+Click and extrude, connect, split, join, orient along the path,
  smooth, straighten, subdivide, draggable curve handles, new paths as a point, line, curve or
  loop, change path type and place objects along a path.
- Lap checkpoints: a Checkpoint box for lap points, numbers kept in order along the lap after
  every edit and on save (by the rule every shipped course follows), checkpoint, start, lap
  check and goal gates drawn apart from the rest, and checks for missing, unnumbered and
  out-of-order checkpoints.
- A Problems panel that checks a course against how every shipped course is built, named undo
  steps, recent courses, an unsaved-changes prompt, and Save and Test.
- Saves LayeredFS mods for Eden-based emulators and exports them for ns2jb-modloader on
  Switch 2, rebuilding the summaries the game reads and growing the resource size table when a
  file outgrows its entry.
- `mkwtrack` library: BYML v7 reading and byte-exact writing, SARC, the resource size table,
  typed JSON, the game catalog, BFRES model and BNTX texture readers and editing operations.
- Tests: unit tests without game files, tests against every shipped file, and `mkw-emutest`,
  which checks mods in an emulator.
