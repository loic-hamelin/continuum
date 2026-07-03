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

pub use graph::{Edge, EdgeKind, VersionedGraph};
pub use node::{NodeId, NodeKind, RailNode};
pub use commit::{Commit, CommitId};
