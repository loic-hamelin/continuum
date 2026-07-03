use crate::node::{NodeId, RailNode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type de relation entre deux nœuds du graphe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    Appartenance,
    DependanceTechnique,
    ContrainteCapacitaire,
    DependanceTemporelle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

/// Un état du graphe versionné : l'ensemble des nœuds et arêtes correspondant
/// à une version (un commit) du système ferroviaire.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionedGraph {
    pub nodes: HashMap<NodeId, RailNode>,
    pub edges: Vec<Edge>,
}

impl VersionedGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: RailNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Comparaison entre deux versions du graphe (diff) : nœuds ajoutés,
    /// supprimés, et modifiés (même id, contenu différent). Sert à la fois
    /// à l'affichage d'un diff et à la détection de conflit lors d'un merge
    /// (voir `Repository::merge`).
    pub fn diff<'a>(&'a self, other: &'a VersionedGraph) -> GraphDiff<'a> {
        let added: Vec<&RailNode> = other
            .nodes
            .values()
            .filter(|n| !self.nodes.contains_key(&n.id))
            .collect();
        let removed: Vec<&RailNode> = self
            .nodes
            .values()
            .filter(|n| !other.nodes.contains_key(&n.id))
            .collect();
        let modified: Vec<(&RailNode, &RailNode)> = self
            .nodes
            .values()
            .filter_map(|before| {
                other.nodes.get(&before.id).and_then(|after| {
                    if before != after {
                        Some((before, after))
                    } else {
                        None
                    }
                })
            })
            .collect();
        GraphDiff {
            added,
            removed,
            modified,
        }
    }

    /// Applique un changement (ajout/suppression de nœuds et d'arêtes) au
    /// graphe courant. Utilisé par `Repository::commit_change` pour faire
    /// évoluer l'état de la pointe d'une branche.
    pub fn apply(&mut self, change: &GraphChange) {
        for id in &change.remove_nodes {
            self.nodes.remove(id);
        }
        for node in &change.add_nodes {
            self.add_node(node.clone());
        }
        self.edges.retain(|edge| !change.remove_edges.contains(edge));
        for edge in &change.add_edges {
            self.add_edge(edge.clone());
        }
    }
}

#[derive(Debug)]
pub struct GraphDiff<'a> {
    pub added: Vec<&'a RailNode>,
    pub removed: Vec<&'a RailNode>,
    /// Paires (état avant, état après) pour chaque nœud présent des deux
    /// côtés mais dont le contenu diffère.
    pub modified: Vec<(&'a RailNode, &'a RailNode)>,
}

/// Un changement à appliquer sur un `VersionedGraph` : la brique de base
/// d'un commit (voir `Repository::commit_change`).
///
/// Construction par chaînage, par ex. :
/// `GraphChange::new().add_node(n).remove_node("voie-3")`.
#[derive(Debug, Clone, Default)]
pub struct GraphChange {
    pub add_nodes: Vec<RailNode>,
    pub remove_nodes: Vec<NodeId>,
    pub add_edges: Vec<Edge>,
    pub remove_edges: Vec<Edge>,
}

impl GraphChange {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(mut self, node: RailNode) -> Self {
        self.add_nodes.push(node);
        self
    }

    pub fn remove_node(mut self, id: impl Into<NodeId>) -> Self {
        self.remove_nodes.push(id.into());
        self
    }

    pub fn add_edge(mut self, edge: Edge) -> Self {
        self.add_edges.push(edge);
        self
    }

    pub fn remove_edge(mut self, edge: Edge) -> Self {
        self.remove_edges.push(edge);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKind;

    #[test]
    fn diff_detects_added_node() {
        let base = VersionedGraph::new();
        let mut branch = VersionedGraph::new();
        branch.add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12 - gare de Lens"));

        let diff = base.diff(&branch);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 0);
        assert_eq!(diff.modified.len(), 0);
    }

    #[test]
    fn diff_detects_modified_node() {
        let mut base = VersionedGraph::new();
        base.add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12 - gare de Lens"));

        let mut branch = base.clone();
        branch.add_node(RailNode::new(
            "voie-12",
            NodeKind::Voie,
            "Voie 12 - gare de Lens (renommée)",
        ));

        let diff = base.diff(&branch);
        assert_eq!(diff.added.len(), 0);
        assert_eq!(diff.removed.len(), 0);
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.modified[0].1.label, "Voie 12 - gare de Lens (renommée)");
    }

    #[test]
    fn apply_adds_and_removes_nodes_and_edges() {
        let mut graph = VersionedGraph::new();
        graph.add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12"));

        let edge = Edge {
            from: "voie-12".to_string(),
            to: "aiguille-45".to_string(),
            kind: EdgeKind::DependanceTechnique,
        };

        let change = GraphChange::new()
            .add_node(RailNode::new(
                "aiguille-45",
                NodeKind::AppareilDeVoie,
                "Aiguille 45",
            ))
            .remove_node("voie-12")
            .add_edge(edge.clone());

        graph.apply(&change);

        assert!(!graph.nodes.contains_key("voie-12"));
        assert!(graph.nodes.contains_key("aiguille-45"));
        assert_eq!(graph.edges, vec![edge]);

        let removal = GraphChange::new().remove_edge(graph.edges[0].clone());
        graph.apply(&removal);
        assert!(graph.edges.is_empty());
    }
}
