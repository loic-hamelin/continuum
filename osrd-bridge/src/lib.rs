//! CONTINUUM — osrd-bridge
//!
//! Pont entre le graphe versionné CONTINUUM et le moteur de simulation
//! open-source OSRD (format de données RailJSON).
//!
//! CONTINUUM ne réimplémente pas la simulation ferroviaire : il s'appuie
//! sur OSRD comme moteur de calcul pour évaluer les conséquences (capacité,
//! robustesse, performance) de chaque branche du graphe. Ce module reste
//! volontairement un squelette : la logique d'export vers RailJSON et
//! d'appel au moteur OSRD reste à construire avec Claude Code, une fois
//! le format d'échange retenu (bibliothèque Rust, appel HTTP à une instance
//! OSRD, ou génération de fichiers RailJSON).

use continuum_graph_engine::VersionedGraph;

/// Convertit un état du graphe CONTINUUM vers le format attendu par OSRD.
/// Non implémenté — point d'entrée à construire.
pub fn export_to_railjson(_graph: &VersionedGraph) -> Result<String, BridgeError> {
    Err(BridgeError::NotImplemented)
}

#[derive(Debug)]
pub enum BridgeError {
    NotImplemented,
}
