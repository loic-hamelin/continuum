//! CONTINUUM — osrd-bridge
//!
//! Pont entre le graphe versionné CONTINUUM et le moteur de simulation
//! open-source OSRD (format de données RailJSON).
//!
//! CONTINUUM ne réimplémente pas la simulation ferroviaire : il s'appuie
//! sur OSRD comme moteur de calcul pour évaluer les conséquences (capacité,
//! robustesse, performance) de chaque branche du graphe.
//!
//! RailJSON ne décrit que l'**infrastructure physique** (voies, appareils
//! de voie, signaux, points opérationnels...). Les sillons, horaires et
//! projets d'investissement de CONTINUUM n'ont pas d'équivalent dans ce
//! format (ce sont d'autres objets côté OSRD : `timetable`,
//! `train_schedule`...) — ils sont donc explicitement ignorés par l'export.
//! Voir `railjson.rs` pour le détail du sous-ensemble modélisé.
//!
//! ## Convention nœud CONTINUUM <-> RailJSON
//!
//! Les champs RailJSON qui n'ont pas de colonne dédiée sur `RailNode` sont
//! lus/écrits dans `RailNode.properties`, sous les clés suivantes :
//!
//! - `Voie` (-> `TrackSection`) : `length`, `geo` (obligatoires),
//!   `slopes`, `curves`, `loading_gauge_limits` (optionnels, `[]` si absents).
//! - `AppareilDeVoie` (-> `Switch`) : `switch_type`, `group_change_delay`,
//!   `ports` (tous obligatoires).
//! - `Signal` (-> `Signal`) : `track`, `position`, `direction`,
//!   `sight_distance` (obligatoires), `logical_signals` (optionnel).

pub mod railjson;

use continuum_graph_engine::{Edge, EdgeKind, NodeKind, RailNode, VersionedGraph};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt;

pub use railjson::*;

/// Erreurs pouvant survenir lors de la conversion entre le graphe
/// CONTINUUM et un document RailJSON.
#[derive(Debug)]
pub enum BridgeError {
    /// Un nœud à exporter n'a pas la propriété attendue par RailJSON pour
    /// son type (voir la convention documentée en tête de ce module).
    MissingField { node_id: String, field: &'static str },
    /// La propriété existe mais son contenu ne correspond pas à ce
    /// qu'attend le champ RailJSON correspondant.
    InvalidField {
        node_id: String,
        field: &'static str,
        reason: String,
    },
    /// Le document fourni n'est pas un JSON valide, ou ne correspond pas
    /// au sous-ensemble de RailJSON couvert par ce module.
    InvalidJson(String),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::MissingField { node_id, field } => {
                write!(f, "nœud '{node_id}' : propriété '{field}' manquante pour l'export RailJSON")
            }
            BridgeError::InvalidField {
                node_id,
                field,
                reason,
            } => write!(
                f,
                "nœud '{node_id}' : propriété '{field}' invalide ({reason})"
            ),
            BridgeError::InvalidJson(reason) => write!(f, "document RailJSON invalide : {reason}"),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<serde_json::Error> for BridgeError {
    fn from(err: serde_json::Error) -> Self {
        BridgeError::InvalidJson(err.to_string())
    }
}

fn get_required<'a>(
    properties: &'a HashMap<String, serde_json::Value>,
    node_id: &str,
    field: &'static str,
) -> Result<&'a serde_json::Value, BridgeError> {
    properties.get(field).ok_or_else(|| BridgeError::MissingField {
        node_id: node_id.to_string(),
        field,
    })
}

fn parse_required<T: DeserializeOwned>(
    properties: &HashMap<String, serde_json::Value>,
    node_id: &str,
    field: &'static str,
) -> Result<T, BridgeError> {
    let value = get_required(properties, node_id, field)?;
    serde_json::from_value(value.clone()).map_err(|err| BridgeError::InvalidField {
        node_id: node_id.to_string(),
        field,
        reason: err.to_string(),
    })
}

fn parse_optional_or_default<T: DeserializeOwned + Default>(
    properties: &HashMap<String, serde_json::Value>,
    field: &str,
) -> T {
    properties
        .get(field)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

/// Convertit un état du graphe CONTINUUM vers un document RailJSON.
///
/// Seuls les nœuds `Voie`, `AppareilDeVoie` et `Signal` sont traduits ;
/// les autres types (`Sillon`, `Horaire`, `ProjetInvestissement`) sont
/// ignorés volontairement (RailJSON ne décrit que l'infrastructure
/// physique, voir le commentaire d'en-tête du module).
pub fn export_to_railjson(graph: &VersionedGraph) -> Result<RailJsonInfra, BridgeError> {
    let mut infra = RailJsonInfra::default();

    for node in graph.nodes.values() {
        match node.kind {
            NodeKind::Voie => {
                let length: f64 = parse_required(&node.properties, &node.id, "length")?;
                let geo = parse_required(&node.properties, &node.id, "geo")?;
                infra.track_sections.push(TrackSection {
                    id: node.id.clone(),
                    length,
                    geo,
                    slopes: parse_optional_or_default(&node.properties, "slopes"),
                    curves: parse_optional_or_default(&node.properties, "curves"),
                    loading_gauge_limits: parse_optional_or_default(
                        &node.properties,
                        "loading_gauge_limits",
                    ),
                });
            }
            NodeKind::AppareilDeVoie => {
                let switch_type: String = parse_required(&node.properties, &node.id, "switch_type")?;
                let group_change_delay: f64 =
                    parse_required(&node.properties, &node.id, "group_change_delay")?;
                let ports = parse_required(&node.properties, &node.id, "ports")?;
                infra.switches.push(Switch {
                    id: node.id.clone(),
                    switch_type,
                    group_change_delay,
                    ports,
                });
            }
            NodeKind::Signal => {
                let track: String = parse_required(&node.properties, &node.id, "track")?;
                let position: f64 = parse_required(&node.properties, &node.id, "position")?;
                let direction: Direction = parse_required(&node.properties, &node.id, "direction")?;
                let sight_distance: f64 =
                    parse_required(&node.properties, &node.id, "sight_distance")?;
                infra.signals.push(railjson::Signal {
                    id: node.id.clone(),
                    track,
                    position,
                    direction,
                    sight_distance,
                    logical_signals: parse_optional_or_default(&node.properties, "logical_signals"),
                });
            }
            NodeKind::Sillon | NodeKind::Horaire | NodeKind::ProjetInvestissement => {
                // RailJSON ne décrit que l'infrastructure physique : les
                // sillons, horaires et projets d'investissement sont
                // d'autres objets côté OSRD (timetable, train schedule...),
                // absents de ce format. On les ignore volontairement.
            }
        }
    }

    // HashMap n'a pas d'ordre stable : on trie par id pour une sortie
    // déterministe (utile notamment pour comparer deux exports en test).
    infra.track_sections.sort_by(|a, b| a.id.cmp(&b.id));
    infra.switches.sort_by(|a, b| a.id.cmp(&b.id));
    infra.signals.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(infra)
}

/// Dérive une position approximative `(track, position_metres)` pour un
/// `Switch`, à partir de son premier port (trié par nom, pour un résultat
/// déterministe) : `position = 0` si le port touche le `BEGIN` de la voie,
/// `= longueur de la voie` si `END`. C'est une approximation assumée — un
/// aiguillage est physiquement à une jonction, pas un point scalaire
/// unique — mais suffisante pour la détection de conflit spatial (voir
/// `graph_engine::compute_merge`). `None` si le port référence une voie
/// introuvable dans le document (données incohérentes).
fn derive_switch_position(switch: &Switch, infra: &RailJsonInfra) -> Option<(String, f64)> {
    let (_, first_port) = switch.ports.iter().min_by_key(|(name, _)| name.as_str())?;
    let track = infra
        .track_sections
        .iter()
        .find(|t| t.id == first_port.track)?;
    let position = match first_port.endpoint {
        Endpoint::Begin => 0.0,
        Endpoint::End => track.length,
    };
    Some((first_port.track.clone(), position))
}

/// Reconstruit un graphe CONTINUUM à partir d'un document RailJSON.
///
/// En plus des propriétés brutes (conservées pour un export ultérieur
/// fidèle), les relations "port -> voie" d'un aiguillage et "signal ->
/// voie" sont matérialisées par des arêtes `DependanceTechnique` : ce
/// n'est pas une donnée dupliquée en trop, juste la même information déjà
/// présente dans les champs RailJSON, rendue visible comme relation dans
/// le graphe CONTINUUM (utile pour le diff et la visualisation).
pub fn import_from_railjson(document: &str) -> Result<VersionedGraph, BridgeError> {
    let infra: RailJsonInfra = serde_json::from_str(document)?;
    let mut graph = VersionedGraph::new();

    for track in &infra.track_sections {
        let mut node = RailNode::new(track.id.clone(), NodeKind::Voie, track.id.clone());
        node.properties
            .insert("length".to_string(), serde_json::to_value(track.length)?);
        node.properties
            .insert("geo".to_string(), serde_json::to_value(&track.geo)?);
        if !track.slopes.is_empty() {
            node.properties
                .insert("slopes".to_string(), serde_json::to_value(&track.slopes)?);
        }
        if !track.curves.is_empty() {
            node.properties
                .insert("curves".to_string(), serde_json::to_value(&track.curves)?);
        }
        if !track.loading_gauge_limits.is_empty() {
            node.properties.insert(
                "loading_gauge_limits".to_string(),
                serde_json::to_value(&track.loading_gauge_limits)?,
            );
        }
        graph.add_node(node);
    }

    for switch in &infra.switches {
        let mut node = RailNode::new(switch.id.clone(), NodeKind::AppareilDeVoie, switch.id.clone());
        node.properties.insert(
            "switch_type".to_string(),
            serde_json::to_value(&switch.switch_type)?,
        );
        node.properties.insert(
            "group_change_delay".to_string(),
            serde_json::to_value(switch.group_change_delay)?,
        );
        node.properties
            .insert("ports".to_string(), serde_json::to_value(&switch.ports)?);
        // `track`/`position` : pas un vrai champ RailJSON pour un Switch
        // (positionné par `ports`, pas par un offset scalaire) — une
        // convention CONTINUUM interne, dérivée du premier port, pour que
        // la détection de conflit spatial (graph-engine) puisse traiter
        // switches et signaux de façon uniforme. Ignorée à l'export
        // (seuls `switch_type`/`group_change_delay`/`ports` sont relus).
        if let Some((track, position)) = derive_switch_position(switch, &infra) {
            node.properties
                .insert("track".to_string(), serde_json::to_value(track)?);
            node.properties
                .insert("position".to_string(), serde_json::to_value(position)?);
        }
        graph.add_node(node);

        for endpoint in switch.ports.values() {
            graph.add_edge(Edge {
                from: switch.id.clone(),
                to: endpoint.track.clone(),
                kind: EdgeKind::DependanceTechnique,
            });
        }
    }

    for signal in &infra.signals {
        let mut node = RailNode::new(signal.id.clone(), NodeKind::Signal, signal.id.clone());
        node.properties
            .insert("track".to_string(), serde_json::to_value(&signal.track)?);
        node.properties
            .insert("position".to_string(), serde_json::to_value(signal.position)?);
        node.properties.insert(
            "direction".to_string(),
            serde_json::to_value(signal.direction)?,
        );
        node.properties.insert(
            "sight_distance".to_string(),
            serde_json::to_value(signal.sight_distance)?,
        );
        if !signal.logical_signals.is_empty() {
            node.properties.insert(
                "logical_signals".to_string(),
                serde_json::to_value(&signal.logical_signals)?,
            );
        }
        graph.add_node(node);

        graph.add_edge(Edge {
            from: signal.id.clone(),
            to: signal.track.clone(),
            kind: EdgeKind::DependanceTechnique,
        });
    }

    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use continuum_graph_engine::NodeKind;

    fn track_node(id: &str, length: f64) -> RailNode {
        let mut node = RailNode::new(id, NodeKind::Voie, id);
        node.properties
            .insert("length".to_string(), serde_json::json!(length));
        node.properties.insert(
            "geo".to_string(),
            serde_json::to_value(LineString::new(vec![
                vec![-0.4, 49.5],
                vec![-0.365, 49.5],
            ]))
            .unwrap(),
        );
        node
    }

    #[test]
    fn export_translates_a_track_section() {
        let mut graph = VersionedGraph::new();
        graph.add_node(track_node("TA0", 2000.0));

        let infra = export_to_railjson(&graph).unwrap();

        assert_eq!(infra.track_sections.len(), 1);
        assert_eq!(infra.track_sections[0].id, "TA0");
        assert_eq!(infra.track_sections[0].length, 2000.0);
        assert!(infra.switches.is_empty());
        assert!(infra.signals.is_empty());
    }

    #[test]
    fn export_ignores_non_infrastructure_node_kinds() {
        let mut graph = VersionedGraph::new();
        graph.add_node(track_node("TA0", 2000.0));
        graph.add_node(RailNode::new("sillon-1", NodeKind::Sillon, "Sillon test"));

        let infra = export_to_railjson(&graph).unwrap();

        assert_eq!(infra.track_sections.len(), 1);
        // Le sillon n'apparaît nulle part dans le document RailJSON.
        assert_eq!(infra.track_sections[0].id, "TA0");
    }

    #[test]
    fn export_fails_with_a_clear_error_on_missing_required_property() {
        let mut graph = VersionedGraph::new();
        graph.add_node(RailNode::new("TA0", NodeKind::Voie, "Voie sans longueur"));

        let err = export_to_railjson(&graph).unwrap_err();
        match err {
            BridgeError::MissingField { node_id, field } => {
                assert_eq!(node_id, "TA0");
                assert_eq!(field, "length");
            }
            other => panic!("attendu MissingField, obtenu {other:?}"),
        }
    }

    #[test]
    fn import_then_export_roundtrips_a_minimal_document() {
        let document = serde_json::json!({
            "version": RAILJSON_VERSION,
            "track_sections": [{
                "id": "TA0",
                "length": 2000.0,
                "geo": { "type": "LineString", "coordinates": [[-0.4, 49.5], [-0.365, 49.5]] },
                "slopes": [],
                "curves": []
            }],
            "switches": [{
                "id": "PA0",
                "switch_type": "point_switch",
                "group_change_delay": 0.0,
                "ports": {
                    "A": { "track": "TA0", "endpoint": "END" }
                }
            }],
            "signals": [{
                "id": "SA0",
                "track": "TA0",
                "position": 1800.0,
                "direction": "START_TO_STOP",
                "sight_distance": 400.0,
                "logical_signals": []
            }],
            "buffer_stops": [], "detectors": [], "electrifications": [],
            "level_crossings": [], "neutral_sections": [], "operational_points": [],
            "routes": [], "speed_sections": []
        })
        .to_string();

        let graph = import_from_railjson(&document).unwrap();
        assert_eq!(graph.nodes.len(), 3);
        assert_eq!(graph.nodes["TA0"].kind, NodeKind::Voie);
        assert_eq!(graph.nodes["PA0"].kind, NodeKind::AppareilDeVoie);
        assert_eq!(graph.nodes["SA0"].kind, NodeKind::Signal);
        // L'aiguillage et le signal pointent bien vers leur voie via une arête.
        assert!(graph
            .edges
            .iter()
            .any(|e| e.from == "PA0" && e.to == "TA0"));
        assert!(graph
            .edges
            .iter()
            .any(|e| e.from == "SA0" && e.to == "TA0"));

        let infra = export_to_railjson(&graph).unwrap();
        assert_eq!(infra.track_sections.len(), 1);
        assert_eq!(infra.switches.len(), 1);
        assert_eq!(infra.signals.len(), 1);
        assert_eq!(infra.track_sections[0].length, 2000.0);
        assert_eq!(infra.signals[0].position, 1800.0);
    }
}
