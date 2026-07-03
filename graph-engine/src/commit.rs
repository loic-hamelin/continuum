use crate::graph::VersionedGraph;

pub type CommitId = String;

/// Un commit : une évolution tracée du graphe, avec son auteur et sa
/// justification. C'est la brique de base de la traçabilité décrite dans
/// docs/theorie.md.
#[derive(Debug, Clone)]
pub struct Commit {
    pub id: CommitId,
    pub parent: Option<CommitId>,
    pub author: String,
    pub message: String,
    /// À remplacer par un vrai type date/heure (ex: crate `chrono`) une fois
    /// les dépendances externes du projet discutées.
    pub timestamp: String,
    pub graph: VersionedGraph,
}
