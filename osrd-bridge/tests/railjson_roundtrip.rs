//! Test de bout en bout avec un vrai jeu de données OSRD ("small_infra"),
//! récupéré depuis le dépôt officiel OSRD :
//! https://github.com/OpenRailAssociation/osrd/blob/dev/tests/data/infras/small_infra/infra.json
//!
//! Vérifie : import -> quelques nœuds attendus sont présents et corrects
//! -> export -> rien n'est perdu parmi ce que CONTINUUM modélise (voies,
//! aiguillages, signaux). Les autres catégories RailJSON (routes,
//! détecteurs, points opérationnels...) sont hors périmètre de cette
//! étape et ne sont donc pas vérifiées ici (voir osrd-bridge/src/lib.rs).

use continuum_graph_engine::NodeKind;
use continuum_osrd_bridge::{export_to_railjson, import_from_railjson, RailJsonInfra};

const SMALL_INFRA: &str = include_str!("fixtures/small_infra.json");

#[test]
fn small_infra_survives_an_import_export_roundtrip() {
    // L'infra originale, pour comparaison directe après l'aller-retour.
    let original: RailJsonInfra =
        serde_json::from_str(SMALL_INFRA).expect("le fixture doit être un RailJSON valide");
    assert_eq!(original.track_sections.len(), 31);
    assert_eq!(original.switches.len(), 17);
    assert_eq!(original.signals.len(), 106);

    // --- Import ---
    let graph = import_from_railjson(SMALL_INFRA).expect("import du small_infra officiel");

    assert_eq!(graph.nodes.len(), 31 + 17 + 106);

    let track = graph.nodes.get("TA0").expect("la voie TA0 doit exister");
    assert_eq!(track.kind, NodeKind::Voie);
    assert_eq!(track.properties["length"], serde_json::json!(2000.0));

    let switch = graph.nodes.get("PA0").expect("l'aiguillage PA0 doit exister");
    assert_eq!(switch.kind, NodeKind::AppareilDeVoie);

    let signal = graph.nodes.get("SA2").expect("le signal SA2 doit exister");
    assert_eq!(signal.kind, NodeKind::Signal);
    assert_eq!(signal.properties["track"], serde_json::json!("TA0"));

    // Les relations port -> voie et signal -> voie doivent être matérialisées.
    assert!(graph.edges.iter().any(|e| e.from == "PA0" && e.to == "TA1"));
    assert!(graph.edges.iter().any(|e| e.from == "SA2" && e.to == "TA0"));

    // --- Export ---
    let reexported = export_to_railjson(&graph).expect("export du graphe importé");

    assert_eq!(reexported.track_sections.len(), 31);
    assert_eq!(reexported.switches.len(), 17);
    assert_eq!(reexported.signals.len(), 106);

    // Rien d'important perdu sur un représentant de chaque type modélisé :
    // comparaison structurelle complète avec l'objet d'origine.
    let original_ta0 = original
        .track_sections
        .iter()
        .find(|t| t.id == "TA0")
        .unwrap();
    let reexported_ta0 = reexported
        .track_sections
        .iter()
        .find(|t| t.id == "TA0")
        .unwrap();
    assert_eq!(original_ta0, reexported_ta0);

    let original_pa0 = original.switches.iter().find(|s| s.id == "PA0").unwrap();
    let reexported_pa0 = reexported.switches.iter().find(|s| s.id == "PA0").unwrap();
    assert_eq!(original_pa0, reexported_pa0);

    let original_sa2 = original.signals.iter().find(|s| s.id == "SA2").unwrap();
    let reexported_sa2 = reexported.signals.iter().find(|s| s.id == "SA2").unwrap();
    assert_eq!(original_sa2, reexported_sa2);
}
