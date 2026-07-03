use crate::node::{NodeId, RailNode};
use std::collections::HashMap;

/// Type de relation entre deux nœuds du graphe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeKind {
    Appartenance,
    DependanceTechnique,
    ContrainteCapacitaire,
    DependanceTemporelle,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

/// Un état du graphe versionné : l'ensemble des nœuds et arêtes correspondant
/// à une version (un commit) du système ferroviaire.
#[derive(Debug, Clone, Default)]
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

    /// Comparaison naïve entre deux versions du graphe (diff).
    /// Point de départ à affiner avec Claude Code : diff structurel des
    /// propriétés, pas seulement présence/absence de nœuds, et détection
    /// des conflits décrite dans docs/theorie.md.
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
        GraphDiff { added, removed }
    }
}

#[derive(Debug)]
pub struct GraphDiff<'a> {
    pub added: Vec<&'a RailNode>,
    pub removed: Vec<&'a RailNode>,
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
    }
}
