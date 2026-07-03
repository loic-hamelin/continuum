use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identifiant unique d'un nœud du graphe.
pub type NodeId = String;

/// Types d'objets ferroviaires représentés dans le graphe.
/// Le graphe est dit "hétérogène" car il mélange plusieurs types de nœuds
/// et de relations (voir docs/theorie.md).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    Voie,
    AppareilDeVoie,
    Signal,
    Sillon,
    Horaire,
    ProjetInvestissement,
}

/// Un objet ferroviaire versionné du graphe.
///
/// `PartialEq` sert à détecter les nœuds modifiés lors d'un diff : deux
/// nœuds avec le même id mais un `PartialEq` différent sont considérés
/// comme "modifiés" plutôt que comme deux nœuds distincts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RailNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    /// Propriétés libres — à structurer plus finement projet par projet
    /// (par ex. un type dédié par NodeKind plutôt qu'un sac de clés/valeurs).
    ///
    /// En `serde_json::Value` plutôt qu'en `String` : certaines propriétés
    /// venant de formats externes (ex: RailJSON côté `osrd-bridge`) sont
    /// elles-mêmes structurées — coordonnées géographiques, listes de
    /// pentes/courbes, ports d'un aiguillage... Un simple texte forcerait
    /// un double encodage JSON-dans-JSON. Les valeurs textuelles restent
    /// utilisables telles quelles (`Value::String`).
    pub properties: HashMap<String, serde_json::Value>,
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
