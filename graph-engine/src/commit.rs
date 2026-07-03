use crate::graph::VersionedGraph;
use chrono::{DateTime, Utc};

pub type CommitId = String;

/// Un commit : une évolution tracée du graphe, avec son auteur et sa
/// justification. C'est la brique de base de la traçabilité décrite dans
/// docs/theorie.md.
///
/// `parents` est une liste plutôt qu'un simple `Option<CommitId>` : un
/// commit normal a un seul parent (zéro pour le commit racine), mais un
/// commit de fusion (merge) en a deux — la branche cible et la branche
/// source. Une seule liste couvre les deux cas sans champ optionnel dédié.
#[derive(Debug, Clone)]
pub struct Commit {
    pub id: CommitId,
    pub parents: Vec<CommitId>,
    pub author: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub graph: VersionedGraph,
}

impl Commit {
    /// Vrai si ce commit est un commit de fusion (deux parents ou plus).
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}
