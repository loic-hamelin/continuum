use crate::commit::{Commit, CommitId};
use crate::graph::{GraphChange, GraphDiff, VersionedGraph};
use crate::node::{NodeId, RailNode};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Nom de la branche de référence créée par défaut à l'initialisation
/// d'un dépôt (l'« état de référence validé » décrit dans le README).
pub const DEFAULT_BRANCH: &str = "reference";

/// Un dépôt : l'ensemble des commits (l'historique complet, pas un seul
/// graphe isolé) et une table de branches (nom -> commit pointé), à la
/// manière de Git.
#[derive(Debug)]
pub struct Repository {
    commits: HashMap<CommitId, Commit>,
    branches: HashMap<String, CommitId>,
    next_commit_seq: u64,
}

impl Repository {
    /// Crée un dépôt avec un commit racine (graphe vide) sur la branche
    /// par défaut `reference`.
    ///
    /// Choix : les ids de commit sont générés séquentiellement ("c1", "c2",
    /// ...) plutôt que par hash de contenu (comme le fait réellement Git
    /// avec SHA-1). C'est plus simple à lire dans les tests et les logs ;
    /// un hash de contenu resterait une évolution possible plus tard si on
    /// veut détecter les doublons ou vérifier l'intégrité du contenu.
    pub fn init(author: impl Into<String>, message: impl Into<String>) -> Self {
        let mut repo = Self {
            commits: HashMap::new(),
            branches: HashMap::new(),
            next_commit_seq: 0,
        };
        let root_id = repo.next_commit_id();
        let root = Commit {
            id: root_id.clone(),
            parents: Vec::new(),
            author: author.into(),
            message: message.into(),
            timestamp: Utc::now(),
            graph: VersionedGraph::new(),
        };
        repo.commits.insert(root_id.clone(), root);
        repo.branches.insert(DEFAULT_BRANCH.to_string(), root_id);
        repo
    }

    fn next_commit_id(&mut self) -> CommitId {
        self.next_commit_seq += 1;
        format!("c{}", self.next_commit_seq)
    }

    /// Le commit pointé par une branche.
    pub fn branch_tip(&self, branch: &str) -> Result<&CommitId, RepositoryError> {
        self.branches
            .get(branch)
            .ok_or_else(|| RepositoryError::BranchNotFound(branch.to_string()))
    }

    pub fn commit(&self, id: &str) -> Result<&Commit, RepositoryError> {
        self.commits
            .get(id)
            .ok_or_else(|| RepositoryError::CommitNotFound(id.to_string()))
    }

    pub fn branch_names(&self) -> impl Iterator<Item = &String> {
        self.branches.keys()
    }

    /// Committer un changement sur une branche : applique `change` à
    /// l'état courant de la branche et crée un nouveau commit dont le
    /// parent est l'ancien tip de cette branche.
    pub fn commit_change(
        &mut self,
        branch: &str,
        change: GraphChange,
        author: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<CommitId, RepositoryError> {
        let parent_id = self.branch_tip(branch)?.clone();
        let mut graph = self.commit(&parent_id)?.graph.clone();
        graph.apply(&change);

        let new_id = self.next_commit_id();
        let commit = Commit {
            id: new_id.clone(),
            parents: vec![parent_id],
            author: author.into(),
            message: message.into(),
            timestamp: Utc::now(),
            graph,
        };
        self.commits.insert(new_id.clone(), commit);
        self.branches.insert(branch.to_string(), new_id.clone());
        Ok(new_id)
    }

    /// Crée une nouvelle branche pointant sur un commit existant. Ne
    /// duplique rien : seul un pointeur (l'id du commit) est enregistré,
    /// l'historique et les graphes restent partagés.
    pub fn create_branch(
        &mut self,
        name: impl Into<String>,
        from_commit: &str,
    ) -> Result<(), RepositoryError> {
        let name = name.into();
        if self.branches.contains_key(&name) {
            return Err(RepositoryError::BranchAlreadyExists(name));
        }
        self.commit(from_commit)?; // vérifie que le commit de départ existe
        self.branches.insert(name, from_commit.to_string());
        Ok(())
    }

    /// Reconstruit l'état complet du graphe à un commit donné (« time
    /// machine »). Chaque commit stocke déjà un instantané complet du
    /// graphe (comme les objets *tree* de Git), donc c'est une lecture
    /// directe — voir `history` pour remonter la chaîne des parents.
    pub fn graph_at(&self, commit_id: &str) -> Result<&VersionedGraph, RepositoryError> {
        Ok(&self.commit(commit_id)?.graph)
    }

    /// Alias explicite de `graph_at`, nommé d'après le vocabulaire de
    /// docs/theorie.md ("time machine").
    pub fn time_machine(&self, commit_id: &str) -> Result<&VersionedGraph, RepositoryError> {
        self.graph_at(commit_id)
    }

    /// Remonte la chaîne des premiers parents depuis `commit_id` jusqu'à
    /// la racine, et renvoie les commits du plus ancien au plus récent —
    /// la séquence de décisions menant à cet état.
    pub fn history(&self, commit_id: &str) -> Result<Vec<&Commit>, RepositoryError> {
        let mut chain = Vec::new();
        let mut current = Some(commit_id.to_string());
        while let Some(id) = current {
            let commit = self.commit(&id)?;
            current = commit.parents.first().cloned();
            chain.push(commit);
        }
        chain.reverse();
        Ok(chain)
    }

    /// Profondeur de tous les ancêtres de `start` (lui-même inclus,
    /// profondeur 0), en remontant tous les parents (un commit de fusion
    /// en a deux). Sert de base au calcul de l'ancêtre commun.
    fn ancestor_depths(&self, start: &str) -> HashMap<CommitId, u32> {
        let mut depths: HashMap<CommitId, u32> = HashMap::new();
        let mut frontier = vec![(start.to_string(), 0u32)];
        while let Some((id, depth)) = frontier.pop() {
            let already_better = matches!(depths.get(&id), Some(&existing) if existing <= depth);
            if already_better {
                continue;
            }
            depths.insert(id.clone(), depth);
            if let Ok(commit) = self.commit(&id) {
                for parent in &commit.parents {
                    frontier.push((parent.clone(), depth + 1));
                }
            }
        }
        depths
    }

    /// Ancêtre commun le plus proche de deux commits (approximation du
    /// "merge-base" de Git) : parmi les commits atteignables depuis les
    /// deux côtés, celui dont la somme des profondeurs aux deux têtes est
    /// minimale.
    fn find_common_ancestor(&self, a: &str, b: &str) -> Option<CommitId> {
        let depths_a = self.ancestor_depths(a);
        let depths_b = self.ancestor_depths(b);
        depths_a
            .iter()
            .filter_map(|(id, &da)| depths_b.get(id).map(|&db| (id.clone(), da + db)))
            .min_by_key(|(_, total)| *total)
            .map(|(id, _)| id)
    }

    /// Fusionne `source` dans `target` : calcule le diff de chaque branche
    /// par rapport à leur ancêtre commun (en réutilisant
    /// `VersionedGraph::diff`), et si aucun nœud n'a été modifié
    /// différemment des deux côtés depuis cet ancêtre, applique les
    /// changements de `source` sur `target` en créant un commit de fusion
    /// à deux parents.
    pub fn merge(
        &mut self,
        source: &str,
        target: &str,
        author: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<CommitId, MergeError> {
        let source_tip = self.branch_tip(source).map_err(branch_not_found)?.clone();
        let target_tip = self.branch_tip(target).map_err(branch_not_found)?.clone();

        let ancestor_id = self
            .find_common_ancestor(&source_tip, &target_tip)
            .ok_or(MergeError::NoCommonAncestor)?;

        let ancestor_graph = self
            .commit(&ancestor_id)
            .expect("l'ancêtre commun doit exister dans le dépôt")
            .graph
            .clone();
        let source_graph = self
            .commit(&source_tip)
            .expect("le tip de la branche source doit exister")
            .graph
            .clone();
        let target_graph = self
            .commit(&target_tip)
            .expect("le tip de la branche cible doit exister")
            .graph
            .clone();

        let diff_source = ancestor_graph.diff(&source_graph);
        let diff_target = ancestor_graph.diff(&target_graph);
        let changed_in_source = changed_node_ids(&diff_source);
        let changed_in_target = changed_node_ids(&diff_target);

        let mut conflicts = Vec::new();
        for id in changed_in_source.intersection(&changed_in_target) {
            let source_state = source_graph.nodes.get(id);
            let target_state = target_graph.nodes.get(id);
            if source_state != target_state {
                conflicts.push(MergeConflict {
                    node_id: id.clone(),
                    ancestor: ancestor_graph.nodes.get(id).cloned(),
                    source: source_state.cloned(),
                    target: target_state.cloned(),
                });
            }
        }
        if !conflicts.is_empty() {
            return Err(MergeError::Conflicts(conflicts));
        }

        let mut merged = target_graph.clone();
        for id in &changed_in_source {
            if changed_in_target.contains(id) {
                continue; // changé des deux côtés sans conflit : donc identiquement, target est déjà à jour
            }
            match source_graph.nodes.get(id) {
                Some(node) => {
                    merged.nodes.insert(id.clone(), node.clone());
                }
                None => {
                    merged.nodes.remove(id);
                }
            }
        }
        for edge in &source_graph.edges {
            if !merged.edges.contains(edge) {
                merged.edges.push(edge.clone());
            }
        }

        let new_id = self.next_commit_id();
        let commit = Commit {
            id: new_id.clone(),
            parents: vec![target_tip, source_tip],
            author: author.into(),
            message: message.into(),
            timestamp: Utc::now(),
            graph: merged,
        };
        self.commits.insert(new_id.clone(), commit);
        self.branches.insert(target.to_string(), new_id.clone());
        Ok(new_id)
    }
}

fn branch_not_found(err: RepositoryError) -> MergeError {
    match err {
        RepositoryError::BranchNotFound(name) => MergeError::BranchNotFound(name),
        other => MergeError::BranchNotFound(other.to_string()),
    }
}

/// Ids des nœuds touchés (ajoutés, supprimés ou modifiés) par un diff.
fn changed_node_ids(diff: &GraphDiff<'_>) -> HashSet<NodeId> {
    diff.added
        .iter()
        .map(|n| n.id.clone())
        .chain(diff.removed.iter().map(|n| n.id.clone()))
        .chain(diff.modified.iter().map(|(_, after)| after.id.clone()))
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum RepositoryError {
    BranchNotFound(String),
    BranchAlreadyExists(String),
    CommitNotFound(CommitId),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepositoryError::BranchNotFound(name) => write!(f, "branche inconnue : {name}"),
            RepositoryError::BranchAlreadyExists(name) => {
                write!(f, "la branche existe déjà : {name}")
            }
            RepositoryError::CommitNotFound(id) => write!(f, "commit inconnu : {id}"),
        }
    }
}

impl std::error::Error for RepositoryError {}

/// Un conflit détecté lors d'un merge : le même nœud a été modifié
/// différemment de part et d'autre depuis l'ancêtre commun.
#[derive(Debug, PartialEq)]
pub struct MergeConflict {
    pub node_id: NodeId,
    pub ancestor: Option<RailNode>,
    pub source: Option<RailNode>,
    pub target: Option<RailNode>,
}

#[derive(Debug, PartialEq)]
pub enum MergeError {
    BranchNotFound(String),
    NoCommonAncestor,
    Conflicts(Vec<MergeConflict>),
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::BranchNotFound(name) => write!(f, "branche inconnue : {name}"),
            MergeError::NoCommonAncestor => write!(f, "aucun ancêtre commun trouvé"),
            MergeError::Conflicts(conflicts) => {
                write!(f, "{} conflit(s) détecté(s)", conflicts.len())
            }
        }
    }
}

impl std::error::Error for MergeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeKind;

    #[test]
    fn init_creates_default_branch_with_root_commit() {
        let repo = Repository::init("loic", "Initialisation du dépôt");
        let tip = repo.branch_tip(DEFAULT_BRANCH).unwrap();
        let root = repo.commit(tip).unwrap();
        assert!(root.parents.is_empty());
        assert!(root.graph.nodes.is_empty());
    }

    #[test]
    fn commit_change_advances_branch_and_keeps_parent_link() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        let root_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();

        let change = GraphChange::new().add_node(RailNode::new(
            "voie-12",
            NodeKind::Voie,
            "Voie 12 - gare de Lens",
        ));
        let new_id = repo
            .commit_change(DEFAULT_BRANCH, change, "loic", "Ajout de la voie 12")
            .unwrap();

        assert_eq!(repo.branch_tip(DEFAULT_BRANCH).unwrap(), &new_id);
        let commit = repo.commit(&new_id).unwrap();
        assert_eq!(commit.parents, vec![root_id]);
        assert_eq!(commit.author, "loic");
        assert!(commit.graph.nodes.contains_key("voie-12"));
    }

    #[test]
    fn create_branch_points_to_existing_commit_without_duplicating_history() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        let root_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();

        repo.create_branch("etude-capacite", &root_id).unwrap();

        assert_eq!(repo.branch_tip("etude-capacite").unwrap(), &root_id);
        // Un seul commit existe toujours : la branche n'a fait que référencer.
        assert_eq!(repo.commits.len(), 1);
    }

    #[test]
    fn merge_without_conflict_combines_both_branches() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        let root_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        repo.create_branch("etude-capacite", &root_id).unwrap();

        repo.commit_change(
            "etude-capacite",
            GraphChange::new().add_node(RailNode::new(
                "aiguille-45",
                NodeKind::AppareilDeVoie,
                "Aiguille 45",
            )),
            "loic",
            "Ajout aiguille 45 sur la branche d'étude",
        )
        .unwrap();

        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new(
                "voie-12",
                NodeKind::Voie,
                "Voie 12 - gare de Lens",
            )),
            "loic",
            "Ajout voie 12 sur la référence",
        )
        .unwrap();

        let merge_id = repo
            .merge("etude-capacite", DEFAULT_BRANCH, "loic", "Merge etude-capacite")
            .unwrap();

        let merge_commit = repo.commit(&merge_id).unwrap();
        assert_eq!(merge_commit.parents.len(), 2);
        assert!(merge_commit.graph.nodes.contains_key("voie-12"));
        assert!(merge_commit.graph.nodes.contains_key("aiguille-45"));
    }

    #[test]
    fn merge_detects_conflict_on_node_modified_differently() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new(
                "voie-12",
                NodeKind::Voie,
                "Voie 12 - gare de Lens",
            )),
            "loic",
            "Ajout voie 12",
        )
        .unwrap();
        let ancestor_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        repo.create_branch("etude-capacite", &ancestor_id).unwrap();

        repo.commit_change(
            "etude-capacite",
            GraphChange::new().add_node(RailNode::new(
                "voie-12",
                NodeKind::Voie,
                "Voie 12 - renommée côté étude",
            )),
            "loic",
            "Renomme voie 12 (branche étude)",
        )
        .unwrap();

        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new(
                "voie-12",
                NodeKind::Voie,
                "Voie 12 - renommée côté référence",
            )),
            "loic",
            "Renomme voie 12 (référence)",
        )
        .unwrap();

        let result = repo.merge("etude-capacite", DEFAULT_BRANCH, "loic", "Merge etude-capacite");

        match result {
            Err(MergeError::Conflicts(conflicts)) => {
                assert_eq!(conflicts.len(), 1);
                assert_eq!(conflicts[0].node_id, "voie-12");
            }
            other => panic!("un conflit était attendu, obtenu : {other:?}"),
        }
    }

    #[test]
    fn time_machine_reconstructs_an_older_state() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        let root_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();

        let after_first = repo
            .commit_change(
                DEFAULT_BRANCH,
                GraphChange::new().add_node(RailNode::new(
                    "voie-12",
                    NodeKind::Voie,
                    "Voie 12 - gare de Lens",
                )),
                "loic",
                "Ajout voie 12",
            )
            .unwrap();

        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new(
                "aiguille-45",
                NodeKind::AppareilDeVoie,
                "Aiguille 45",
            )),
            "loic",
            "Ajout aiguille 45",
        )
        .unwrap();

        // L'état courant contient les deux nœuds...
        let current = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        assert_eq!(repo.graph_at(&current).unwrap().nodes.len(), 2);

        // ...mais l'état au commit précédent n'en contient qu'un.
        let older = repo.time_machine(&after_first).unwrap();
        assert_eq!(older.nodes.len(), 1);
        assert!(older.nodes.contains_key("voie-12"));

        // Et l'état racine est bien vide.
        let root_state = repo.time_machine(&root_id).unwrap();
        assert!(root_state.nodes.is_empty());

        // history() doit remonter la chaîne dans l'ordre chronologique.
        let chain = repo.history(&current).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].id, root_id);
        assert_eq!(chain[2].id, current);
    }
}
