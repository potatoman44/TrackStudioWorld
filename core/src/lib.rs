//! Mario Kart World course data: the file formats, what the game expects of them, and the
//! editing operations of TrackStudioWorld.
//!
//! * [`byml`], [`sarc`], [`restbl`], [`typedjson`], [`io`]: file formats.
//! * [`banc`]: course files and the summaries the game reads from them.
//! * [`workspace`]: loading courses and saving them as LayeredFS mods.
//! * [`catalog`]: object types, parameter tables, path types and stage placements.
//! * [`doc`]: an open course with undo and redo; [`tools`]: copy and paste, path drawing and
//!   reshaping, placing along paths; [`checkpoints`]: lap checkpoint numbering; [`check`]: the
//!   course checks the Problems panel lists.
//! * [`bfres`], [`bntx`], [`model`]: the game's models and textures, read from the romfs.
//! * [`import`]: OBJ, COLLADA and FBX models and their images, from other tools.
//! * [`math`], [`raycast`], [`scene`]: transforms, picking and what the viewport draws.

pub mod banc;
pub mod bfres;
pub mod bfres_write;
pub mod bntx;
pub mod byml;
pub mod catalog;
pub mod check;
pub mod checkpoints;
pub mod custom;
pub mod doc;
pub mod error;
pub mod import;
pub mod io;
pub mod math;
pub mod model;
pub mod raycast;
pub mod restbl;
pub mod sarc;
pub mod scene;
pub mod tools;
pub mod typedjson;
pub mod workspace;

pub use error::{Error, Result};
