//! CONTINUUM — cli
//!
//! Interface en ligne de commande, point d'entrée pour manipuler le graphe
//! versionné (à terme : créer des branches, committer, comparer, fusionner).
//!
//! Cette version affiche une démonstration minimale : création d'un petit
//! graphe, ajout d'une branche avec un nœud supplémentaire, et affichage
//! du diff entre les deux. À développer avec Claude Code.

use continuum_graph_engine::{NodeKind, RailNode, VersionedGraph};

fn main() {
    let mut reference = VersionedGraph::new();
    reference.add_node(RailNode::new(
        "voie-12",
        NodeKind::Voie,
        "Voie 12 - gare de Lens",
    ));

    let mut branche_etude = reference.clone();
    branche_etude.add_node(RailNode::new(
        "aiguille-45",
        NodeKind::AppareilDeVoie,
        "Aiguille 45 (hypothèse d'ajout - branche 'etude-capacite')",
    ));

    let diff = reference.diff(&branche_etude);

    println!("CONTINUUM — démonstration minimale du moteur de graphe\n");
    println!("État de référence : {} nœud(s)", reference.nodes.len());
    println!("Branche 'etude-capacite' : {} nœud(s)", branche_etude.nodes.len());
    println!("\nDiff (référence -> branche) :");
    for node in diff.added {
        println!("  + {} ({:?}) — {}", node.id, node.kind, node.label);
    }
    for node in diff.removed {
        println!("  - {} ({:?}) — {}", node.id, node.kind, node.label);
    }
}
