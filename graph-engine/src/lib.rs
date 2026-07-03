//! CONTINUUM — graph-engine
//!
//! Cœur du modèle de graphe versionné hétérogène du système ferroviaire.
//! Ce module pose les fondations : les objets ferroviaires (nœuds), leurs
//! relations (arêtes), et l'historique de leurs versions (commits).
//!
//! Point de départ volontairement simple — à faire évoluer avec Claude Code
//! au fur et à mesure (voir docs/theorie.md pour le cadre conceptuel complet).

pub mod node;
pub mod graph;
pub mod commit;
pub mod repository;

pub use graph::{Edge, EdgeKind, GraphChange, GraphDiff, VersionedGraph};
pub use node::{NodeId, NodeKind, RailNode};
pub use commit::{Commit, CommitId};
pub use repository::{
    changed_node_ids, compute_merge, resolve_merge, ConflictResolution, ConflictSide,
    MergeConflict, MergeError, Repository, RepositoryError, ResolutionSide, DEFAULT_BRANCH,
    DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS,
};
