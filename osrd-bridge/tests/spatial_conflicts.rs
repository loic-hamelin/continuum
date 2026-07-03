//! Test de bout en bout de la détection de conflit spatial, construit sur
//! le vrai jeu de données OSRD "small_infra" : import -> deux branches qui
//! ajoutent chacune un nouvel aiguillage sur la même vraie voie (`TA0`), à
//! quelques mètres d'écart -> fusion -> conflit spatial détecté.
//!
//! Un diff par identifiant ne verrait jamais ce problème (deux ids
//! différents, chacun "juste ajouté" de son côté) — c'est précisément ce
//! que `graph_engine::compute_merge` est censé attraper.

use continuum_graph_engine::{
    GraphChange, MergeConflict, MergeError, NodeKind, RailNode, Repository, DEFAULT_BRANCH,
};
use continuum_osrd_bridge::import_from_railjson;

const SMALL_INFRA: &str = include_str!("fixtures/small_infra.json");

fn point_node(id: &str, track: &str, position: f64) -> RailNode {
    let mut node = RailNode::new(id, NodeKind::AppareilDeVoie, id);
    node.properties
        .insert("track".to_string(), serde_json::json!(track));
    node.properties
        .insert("position".to_string(), serde_json::json!(position));
    node
}

#[test]
fn two_switches_added_close_together_on_ta0_conflict_on_merge() {
    // Commit racine = la vraie infrastructure importée depuis small_infra.json,
    // appliquée comme premier changement sur la branche par défaut (elle-même
    // créée vide par `Repository::init`).
    let reference_graph =
        import_from_railjson(SMALL_INFRA).expect("import du small_infra officiel");
    let mut repo = Repository::init("loic", "Initialisation depuis small_infra");
    let mut change = GraphChange::new();
    for node in reference_graph.nodes.values().cloned() {
        change = change.add_node(node);
    }
    for edge in reference_graph.edges.clone() {
        change = change.add_edge(edge);
    }
    repo.commit_change(DEFAULT_BRANCH, change, "system", "Import small_infra")
        .unwrap();

    let ancestor_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
    repo.create_branch("etude-capacite", &ancestor_id).unwrap();

    // Branche "etude-capacite" : un nouvel aiguillage à 100m sur TA0.
    repo.commit_change(
        "etude-capacite",
        GraphChange::new().add_node(point_node("aiguille-etude", "TA0", 100.0)),
        "loic",
        "Ajout aiguillage hypothèse (étude de capacité)",
    )
    .unwrap();

    // Branche de référence : un autre aiguillage, 5m plus loin sur la même voie.
    repo.commit_change(
        DEFAULT_BRANCH,
        GraphChange::new().add_node(point_node("aiguille-reference", "TA0", 105.0)),
        "loic",
        "Ajout aiguillage (référence)",
    )
    .unwrap();

    let result = repo.merge("etude-capacite", DEFAULT_BRANCH, "loic", "Merge etude-capacite");

    match result {
        Err(MergeError::Conflicts(conflicts)) => {
            let spatial: Vec<&MergeConflict> = conflicts
                .iter()
                .filter(|c| matches!(c, MergeConflict::Spatial { .. }))
                .collect();
            assert_eq!(
                spatial.len(),
                1,
                "un seul conflit spatial attendu, obtenu : {conflicts:?}"
            );
            match spatial[0] {
                MergeConflict::Spatial {
                    track,
                    distance_meters,
                    ..
                } => {
                    assert_eq!(track, "TA0");
                    assert!((*distance_meters - 5.0).abs() < f64::EPSILON);
                }
                _ => unreachable!(),
            }
        }
        other => panic!("un conflit spatial était attendu, obtenu : {other:?}"),
    }
}
