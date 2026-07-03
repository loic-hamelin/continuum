//! CONTINUUM — schema-engine
//!
//! Extraction d'un sous-graphe topologique ferroviaire à partir d'une
//! infrastructure RailJSON et d'une sélection géographique de
//! l'utilisateur (rectangle pour l'instant, lasso ensuite). Ce crate ne
//! fait volontairement que l'extraction — le layout schématique (mise en
//! forme orthogonale/topologique) viendra dans une étape séparée, une
//! fois l'extraction validée.

pub mod extract;
pub mod geometry;
pub mod layout;
pub mod railjson;

pub use extract::{extract_bbox, ExtractedSchema, ExtractedTrackSegment};
pub use geometry::Bbox;
pub use layout::{compute_layout, LayoutEdge, LayoutNode, SchematicLayout};
pub use railjson::RailInfra;
