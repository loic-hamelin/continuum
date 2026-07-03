use std::collections::HashMap;

/// Identifiant unique d'un nœud du graphe.
pub type NodeId = String;

/// Types d'objets ferroviaires représentés dans le graphe.
/// Le graphe est dit "hétérogène" car il mélange plusieurs types de nœuds
/// et de relations (voir docs/theorie.md).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Voie,
    AppareilDeVoie,
    Signal,
    Sillon,
    Horaire,
    ProjetInvestissement,
}

/// Un objet ferroviaire versionné du graphe.
#[derive(Debug, Clone)]
pub struct RailNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    /// Propriétés libres — à structurer plus finement projet par projet
    /// (par ex. un type dédié par NodeKind plutôt qu'un sac de clés/valeurs).
    pub properties: HashMap<String, String>,
}

impl RailNode {
    pub fn new(id: impl Into<NodeId>, kind: NodeKind, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            label: label.into(),
            properties: HashMap::new(),
        }
    }
}
