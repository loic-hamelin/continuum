use crate::commit::{Commit, CommitId};
use crate::graph::{GraphChange, GraphDiff, VersionedGraph};
use crate::node::{NodeId, RailNode};
use chrono::Utc;
use serde::{Deserialize, Serialize};
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

    /// Fusionne `source` dans `target`, avec le seuil de détection de
    /// conflit spatial par défaut (voir `DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS`).
    pub fn merge(
        &mut self,
        source: &str,
        target: &str,
        author: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<CommitId, MergeError> {
        self.merge_with_threshold(
            source,
            target,
            author,
            message,
            DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS,
        )
    }

    /// Fusionne `source` dans `target` : calcule le diff de chaque branche
    /// par rapport à leur ancêtre commun, et si `compute_merge` ne détecte
    /// aucun conflit, crée un commit de fusion à deux parents avec le
    /// graphe résultant. `spatial_threshold_meters` contrôle la distance
    /// en-deçà de laquelle deux objets ponctuels différents sur la même
    /// voie sont considérés en conflit spatial (voir `compute_merge`).
    pub fn merge_with_threshold(
        &mut self,
        source: &str,
        target: &str,
        author: impl Into<String>,
        message: impl Into<String>,
        spatial_threshold_meters: f64,
    ) -> Result<CommitId, MergeError> {
        let (target_tip, source_tip, ancestor_graph, source_graph, target_graph) =
            self.load_merge_graphs(source, target)?;

        let merged = compute_merge(
            &ancestor_graph,
            &source_graph,
            &target_graph,
            spatial_threshold_meters,
        )
        .map_err(MergeError::Conflicts)?;

        Ok(self.commit_merge_result(target, target_tip, source_tip, author, message, merged))
    }

    /// Fusionne `source` dans `target` en appliquant des résolutions
    /// choisies par l'utilisateur pour chaque conflit détecté (voir
    /// `resolve_merge`) — l'action "Valider la fusion" de l'interface,
    /// après un premier appel à `merge`/`merge_with_threshold` qui a
    /// renvoyé des conflits. Les conflits sont recalculés à partir de
    /// l'état courant des branches (pas de la liste transmise par
    /// l'appelant), pour éviter une fusion incohérente si les branches ont
    /// bougé entre-temps.
    pub fn merge_resolve(
        &mut self,
        source: &str,
        target: &str,
        author: impl Into<String>,
        message: impl Into<String>,
        resolutions: &[ConflictResolution],
        spatial_threshold_meters: f64,
    ) -> Result<CommitId, MergeError> {
        let (target_tip, source_tip, ancestor_graph, source_graph, target_graph) =
            self.load_merge_graphs(source, target)?;

        let merged = resolve_merge(
            &ancestor_graph,
            &source_graph,
            &target_graph,
            resolutions,
            spatial_threshold_meters,
        )
        .map_err(MergeError::Conflicts)?;

        Ok(self.commit_merge_result(target, target_tip, source_tip, author, message, merged))
    }

    /// Résout les tips des deux branches, leur ancêtre commun, et charge
    /// les 3 instantanés de graphe correspondants — la partie commune à
    /// `merge_with_threshold` et `merge_resolve`.
    fn load_merge_graphs(
        &self,
        source: &str,
        target: &str,
    ) -> Result<(CommitId, CommitId, VersionedGraph, VersionedGraph, VersionedGraph), MergeError>
    {
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

        Ok((target_tip, source_tip, ancestor_graph, source_graph, target_graph))
    }

    /// Crée le commit de fusion (deux parents) à partir du graphe déjà
    /// résolu, et avance la branche cible dessus.
    fn commit_merge_result(
        &mut self,
        target: &str,
        target_tip: CommitId,
        source_tip: CommitId,
        author: impl Into<String>,
        message: impl Into<String>,
        merged: VersionedGraph,
    ) -> CommitId {
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
        new_id
    }
}

fn branch_not_found(err: RepositoryError) -> MergeError {
    match err {
        RepositoryError::BranchNotFound(name) => MergeError::BranchNotFound(name),
        other => MergeError::BranchNotFound(other.to_string()),
    }
}

/// Ids des nœuds touchés (ajoutés, supprimés ou modifiés) par un diff.
///
/// `pub` : réutilisé tel quel par `api::db` qui porte ce même algorithme de
/// fusion vers une persistance SQL (voir `api/src/db.rs`), pour ne pas
/// dupliquer la définition de "changé" entre les deux implémentations.
pub fn changed_node_ids(diff: &GraphDiff<'_>) -> HashSet<NodeId> {
    diff.added
        .iter()
        .map(|n| n.id.clone())
        .chain(diff.removed.iter().map(|n| n.id.clone()))
        .chain(diff.modified.iter().map(|(_, after)| after.id.clone()))
        .collect()
}

/// Distance par défaut (en mètres) en-deçà de laquelle deux objets
/// ponctuels différents, positionnés sur la même voie, sont considérés
/// comme un conflit spatial (voir `MergeConflict::Spatial`).
pub const DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS: f64 = 8.0;

/// Lit la position d'un objet ponctuel le long d'une voie, à partir des
/// propriétés `track` (chaîne) et `position` (nombre) — la convention déjà
/// utilisée par `osrd-bridge` pour les `Signal`, et étendue aux `Switch`
/// (dérivée de leur premier port, voir `osrd-bridge::import_from_railjson`).
/// Volontairement générique sur `NodeKind` : ne présuppose pas quels types
/// de nœuds portent une position, juste la présence de ces deux clés.
fn point_position(node: &RailNode) -> Option<(NodeId, f64)> {
    let track = node.properties.get("track")?.as_str()?.to_string();
    let position = node.properties.get("position")?.as_f64()?;
    Some((track, position))
}

/// Calcule le résultat d'une fusion entre `source` et `target`, par
/// rapport à leur `ancestor` commun : soit le graphe fusionné, soit la
/// liste des conflits qui l'en empêchent. Fonction pure (aucun accès à un
/// historique de commits) — réutilisée à la fois par `Repository::merge`
/// (en mémoire) et par `api::db::merge_branches` (persistance SQL), pour
/// ne pas dupliquer cette logique entre les deux implémentations.
///
/// Trois types de conflit sont détectés :
/// - `Modification` : même id, modifié différemment des deux côtés (avec
///   ou sans ancêtre commun — deux branches peuvent aussi avoir ajouté le
///   même id indépendamment, avec un contenu différent).
/// - `DeletionVsModification` : un côté a supprimé l'objet, l'autre l'a
///   modifié — un cas de conflit à part entière, distinct d'une simple
///   modification concurrente.
/// - `Spatial` : deux ids *différents*, chacun ajouté/modifié dans une
///   branche différente, positionnés sur la même voie à moins de
///   `spatial_threshold_meters` l'un de l'autre. Un diff Git classique ne
///   détecterait jamais ce cas puisqu'il ne compare que les identifiants.
pub fn compute_merge(
    ancestor: &VersionedGraph,
    source: &VersionedGraph,
    target: &VersionedGraph,
    spatial_threshold_meters: f64,
) -> Result<VersionedGraph, Vec<MergeConflict>> {
    let diff_source = ancestor.diff(source);
    let diff_target = ancestor.diff(target);
    let changed_in_source = changed_node_ids(&diff_source);
    let changed_in_target = changed_node_ids(&diff_target);

    let mut conflicts = Vec::new();

    // --- Conflits sur un même id : modification concurrente, ou
    // suppression d'un côté face à une modification de l'autre. ---
    for id in changed_in_source.intersection(&changed_in_target) {
        let source_state = source.nodes.get(id);
        let target_state = target.nodes.get(id);
        if source_state == target_state {
            continue; // changé identiquement des deux côtés : pas de conflit
        }
        match (source_state, target_state) {
            (None, Some(modified)) => conflicts.push(MergeConflict::DeletionVsModification {
                node_id: id.clone(),
                ancestor: ancestor
                    .nodes
                    .get(id)
                    .cloned()
                    .expect("un nœud supprimé doit avoir existé dans l'ancêtre"),
                modified: modified.clone(),
                deleted_in: ConflictSide::Source,
            }),
            (Some(modified), None) => conflicts.push(MergeConflict::DeletionVsModification {
                node_id: id.clone(),
                ancestor: ancestor
                    .nodes
                    .get(id)
                    .cloned()
                    .expect("un nœud supprimé doit avoir existé dans l'ancêtre"),
                modified: modified.clone(),
                deleted_in: ConflictSide::Target,
            }),
            (Some(s), Some(t)) => conflicts.push(MergeConflict::Modification {
                node_id: id.clone(),
                ancestor: ancestor.nodes.get(id).cloned(),
                source: s.clone(),
                target: t.clone(),
            }),
            (None, None) => unreachable!("source_state != target_state exclut ce cas"),
        }
    }

    // --- Conflits spatiaux : ids différents, positions proches sur la
    // même voie, chacun changé dans une branche différente. ---
    for id_a in &changed_in_source {
        let Some(node_a) = source.nodes.get(id_a) else {
            continue; // supprimé côté source : pas de position à comparer
        };
        let Some((track_a, position_a)) = point_position(node_a) else {
            continue; // pas un objet ponctuel (pas de track/position)
        };
        for id_b in &changed_in_target {
            if id_a == id_b {
                continue; // même id : déjà traité ci-dessus
            }
            let Some(node_b) = target.nodes.get(id_b) else {
                continue;
            };
            let Some((track_b, position_b)) = point_position(node_b) else {
                continue;
            };
            if track_a != track_b {
                continue;
            }
            let distance = (position_a - position_b).abs();
            if distance <= spatial_threshold_meters {
                conflicts.push(MergeConflict::Spatial {
                    track: track_a.clone(),
                    source_node: node_a.clone(),
                    source_position: position_a,
                    target_node: node_b.clone(),
                    target_position: position_b,
                    distance_meters: distance,
                });
            }
        }
    }

    if !conflicts.is_empty() {
        return Err(conflicts);
    }

    let mut merged = target.clone();
    for id in &changed_in_source {
        if changed_in_target.contains(id) {
            continue; // changé des deux côtés sans conflit : donc identiquement, target est déjà à jour
        }
        match source.nodes.get(id) {
            Some(node) => {
                merged.nodes.insert(id.clone(), node.clone());
            }
            None => {
                merged.nodes.remove(id);
            }
        }
    }
    for edge in &source.edges {
        if !merged.edges.contains(edge) {
            merged.edges.push(edge.clone());
        }
    }

    Ok(merged)
}

/// Quel côté choisir pour résoudre un conflit — voir `ConflictResolution`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionSide {
    Source,
    Target,
}

/// La résolution choisie par l'utilisateur pour un conflit donné —
/// identifie le conflit visé (même clé que la variante `MergeConflict`
/// correspondante) et le côté à garder. Pour `Spatial`, "garder la
/// source" retire l'objet de la cible et inversement, puisque les deux
/// objets sont distincts (contrairement aux deux autres variantes, où
/// c'est le même id des deux côtés).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConflictResolution {
    Modification { node_id: NodeId, keep: ResolutionSide },
    DeletionVsModification { node_id: NodeId, keep: ResolutionSide },
    Spatial {
        source_node_id: NodeId,
        target_node_id: NodeId,
        keep: ResolutionSide,
    },
}

fn resolution_matches(resolution: &ConflictResolution, conflict: &MergeConflict) -> bool {
    match (resolution, conflict) {
        (
            ConflictResolution::Modification { node_id: r, .. },
            MergeConflict::Modification { node_id: c, .. },
        ) => r == c,
        (
            ConflictResolution::DeletionVsModification { node_id: r, .. },
            MergeConflict::DeletionVsModification { node_id: c, .. },
        ) => r == c,
        (
            ConflictResolution::Spatial {
                source_node_id,
                target_node_id,
                ..
            },
            MergeConflict::Spatial {
                source_node,
                target_node,
                ..
            },
        ) => source_node_id == &source_node.id && target_node_id == &target_node.id,
        _ => false,
    }
}

fn apply_resolution(
    merged: &mut VersionedGraph,
    source: &VersionedGraph,
    target: &VersionedGraph,
    conflict: &MergeConflict,
    resolution: &ConflictResolution,
) {
    match (conflict, resolution) {
        (
            MergeConflict::Modification { node_id, .. },
            ConflictResolution::Modification { keep, .. },
        )
        | (
            MergeConflict::DeletionVsModification { node_id, .. },
            ConflictResolution::DeletionVsModification { keep, .. },
        ) => {
            let chosen = match keep {
                ResolutionSide::Source => source,
                ResolutionSide::Target => target,
            };
            match chosen.nodes.get(node_id) {
                Some(node) => {
                    merged.nodes.insert(node_id.clone(), node.clone());
                }
                None => {
                    merged.nodes.remove(node_id);
                }
            }
        }
        (
            MergeConflict::Spatial {
                source_node,
                target_node,
                ..
            },
            ConflictResolution::Spatial { keep, .. },
        ) => match keep {
            ResolutionSide::Source => {
                merged.nodes.remove(&target_node.id);
                merged.nodes.insert(source_node.id.clone(), source_node.clone());
            }
            ResolutionSide::Target => {
                // `target_node` est déjà présent dans `merged` (cloné
                // depuis `target` en tout début de fusion) : rien à faire
                // à part ne pas insérer `source_node`.
                merged.nodes.remove(&source_node.id);
            }
        },
        _ => unreachable!("resolution_matches garantit la cohérence entre kind de conflit et de résolution"),
    }
}

/// Applique des résolutions choisies par l'utilisateur aux conflits d'une
/// fusion, et produit le graphe résultant.
///
/// Recalcule les conflits à partir de l'état courant de `ancestor`,
/// `source` et `target` (plutôt que de faire confiance à une liste de
/// conflits transmise par l'appelant) : si les branches ont bougé entre la
/// prévisualisation d'un merge et la validation des résolutions, la
/// fusion reste cohérente avec l'état réel plutôt qu'avec un instantané
/// périmé. S'il manque une résolution pour un conflit toujours présent,
/// renvoie la liste des conflits non couverts.
pub fn resolve_merge(
    ancestor: &VersionedGraph,
    source: &VersionedGraph,
    target: &VersionedGraph,
    resolutions: &[ConflictResolution],
    spatial_threshold_meters: f64,
) -> Result<VersionedGraph, Vec<MergeConflict>> {
    let conflicts = match compute_merge(ancestor, source, target, spatial_threshold_meters) {
        Ok(merged) => return Ok(merged), // plus de conflit : les résolutions sont sans objet
        Err(conflicts) => conflicts,
    };

    let diff_source = ancestor.diff(source);
    let changed_in_source = changed_node_ids(&diff_source);

    // Ids concernés par un conflit : traités explicitement via une
    // résolution ci-dessous, pas par la passe "changements sans conflit".
    let mut conflicting_ids: HashSet<NodeId> = HashSet::new();
    for conflict in &conflicts {
        match conflict {
            MergeConflict::Modification { node_id, .. }
            | MergeConflict::DeletionVsModification { node_id, .. } => {
                conflicting_ids.insert(node_id.clone());
            }
            MergeConflict::Spatial {
                source_node,
                target_node,
                ..
            } => {
                conflicting_ids.insert(source_node.id.clone());
                conflicting_ids.insert(target_node.id.clone());
            }
        }
    }

    let mut merged = target.clone();
    for id in &changed_in_source {
        if conflicting_ids.contains(id) {
            continue;
        }
        match source.nodes.get(id) {
            Some(node) => {
                merged.nodes.insert(id.clone(), node.clone());
            }
            None => {
                merged.nodes.remove(id);
            }
        }
    }
    for edge in &source.edges {
        if !merged.edges.contains(edge) {
            merged.edges.push(edge.clone());
        }
    }

    let mut missing = Vec::new();
    for conflict in &conflicts {
        match resolutions.iter().find(|r| resolution_matches(r, conflict)) {
            Some(resolution) => apply_resolution(&mut merged, source, target, conflict, resolution),
            None => missing.push(conflict.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(missing);
    }

    Ok(merged)
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

/// De quel côté un nœud a été supprimé, dans un conflit
/// `DeletionVsModification`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictSide {
    Source,
    Target,
}

/// Un conflit détecté lors d'un merge — voir `compute_merge` pour le
/// détail de la détection de chaque variante.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MergeConflict {
    /// Le même nœud a été modifié différemment des deux côtés (aucune
    /// suppression impliquée). `ancestor` est `None` si les deux branches
    /// ont ajouté ce même id indépendamment, avec un contenu différent.
    Modification {
        node_id: NodeId,
        ancestor: Option<RailNode>,
        source: RailNode,
        target: RailNode,
    },
    /// Un côté a supprimé le nœud, l'autre l'a modifié.
    DeletionVsModification {
        node_id: NodeId,
        ancestor: RailNode,
        /// Le nœud tel que modifié par le côté qui ne l'a pas supprimé.
        modified: RailNode,
        deleted_in: ConflictSide,
    },
    /// Deux nœuds *différents*, ajoutés/modifiés chacun dans une branche
    /// différente, mais positionnés sur la même voie à moins de
    /// `distance_meters` l'un de l'autre — un conflit spécifique au
    /// ferroviaire qu'un diff par id ne détecterait jamais.
    Spatial {
        track: NodeId,
        source_node: RailNode,
        source_position: f64,
        target_node: RailNode,
        target_position: f64,
        distance_meters: f64,
    },
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
                match &conflicts[0] {
                    MergeConflict::Modification { node_id, .. } => assert_eq!(node_id, "voie-12"),
                    other => panic!("attendu Modification, obtenu {other:?}"),
                }
            }
            other => panic!("un conflit était attendu, obtenu : {other:?}"),
        }
    }

    #[test]
    fn merge_detects_conflict_when_one_side_deletes_and_the_other_modifies() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12")),
            "loic",
            "Ajout voie 12",
        )
        .unwrap();
        let ancestor_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        repo.create_branch("etude-capacite", &ancestor_id).unwrap();

        // Côté étude : suppression de la voie.
        repo.commit_change(
            "etude-capacite",
            GraphChange::new().remove_node("voie-12"),
            "loic",
            "Suppression voie 12",
        )
        .unwrap();

        // Côté référence : modification de la même voie.
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12 renommée")),
            "loic",
            "Renomme voie 12",
        )
        .unwrap();

        let result = repo.merge("etude-capacite", DEFAULT_BRANCH, "loic", "Merge etude-capacite");

        match result {
            Err(MergeError::Conflicts(conflicts)) => {
                assert_eq!(conflicts.len(), 1);
                match &conflicts[0] {
                    MergeConflict::DeletionVsModification {
                        node_id,
                        deleted_in,
                        ..
                    } => {
                        assert_eq!(node_id, "voie-12");
                        assert_eq!(*deleted_in, ConflictSide::Source);
                    }
                    other => panic!("attendu DeletionVsModification, obtenu {other:?}"),
                }
            }
            other => panic!("un conflit était attendu, obtenu : {other:?}"),
        }
    }

    /// Construit un nœud ponctuel (aiguillage, signal...) positionné le
    /// long d'une voie — reprend la convention `track`/`position` utilisée
    /// par `osrd-bridge` (voir `point_position` dans ce module).
    fn point_node(id: &str, track: &str, position: f64) -> RailNode {
        let mut node = RailNode::new(id, NodeKind::AppareilDeVoie, id);
        node.properties
            .insert("track".to_string(), serde_json::json!(track));
        node.properties
            .insert("position".to_string(), serde_json::json!(position));
        node
    }

    #[test]
    fn merge_detects_spatial_conflict_between_two_new_objects_close_on_the_same_track() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("TA0", NodeKind::Voie, "Voie A")),
            "loic",
            "Ajout voie TA0",
        )
        .unwrap();
        let ancestor_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        repo.create_branch("etude-capacite", &ancestor_id).unwrap();

        // Deux aiguillages différents, ajoutés indépendamment à 4m d'écart
        // sur la même voie TA0 — aucun diff par id ne verrait de problème.
        repo.commit_change(
            "etude-capacite",
            GraphChange::new().add_node(point_node("aiguille-a", "TA0", 100.0)),
            "loic",
            "Ajout aiguille A (étude)",
        )
        .unwrap();
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(point_node("aiguille-b", "TA0", 104.0)),
            "loic",
            "Ajout aiguille B (référence)",
        )
        .unwrap();

        let result = repo.merge("etude-capacite", DEFAULT_BRANCH, "loic", "Merge etude-capacite");

        match result {
            Err(MergeError::Conflicts(conflicts)) => {
                assert_eq!(conflicts.len(), 1);
                match &conflicts[0] {
                    MergeConflict::Spatial {
                        track,
                        distance_meters,
                        ..
                    } => {
                        assert_eq!(track, "TA0");
                        assert!((*distance_meters - 4.0).abs() < f64::EPSILON);
                    }
                    other => panic!("attendu Spatial, obtenu {other:?}"),
                }
            }
            other => panic!("un conflit spatial était attendu, obtenu : {other:?}"),
        }
    }

    #[test]
    fn merge_does_not_flag_distant_objects_on_the_same_track_as_spatial_conflict() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("TA0", NodeKind::Voie, "Voie A")),
            "loic",
            "Ajout voie TA0",
        )
        .unwrap();
        let ancestor_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        repo.create_branch("etude-capacite", &ancestor_id).unwrap();

        // Même montage, mais à 400m d'écart : largement au-delà du seuil
        // par défaut (8m) — pas de conflit spatial.
        repo.commit_change(
            "etude-capacite",
            GraphChange::new().add_node(point_node("aiguille-a", "TA0", 100.0)),
            "loic",
            "Ajout aiguille A (étude)",
        )
        .unwrap();
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(point_node("aiguille-b", "TA0", 500.0)),
            "loic",
            "Ajout aiguille B (référence)",
        )
        .unwrap();

        let result = repo.merge("etude-capacite", DEFAULT_BRANCH, "loic", "Merge etude-capacite");
        assert!(result.is_ok());
    }

    #[test]
    fn merge_resolve_applies_chosen_side_for_a_modification_conflict() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12")),
            "loic",
            "Ajout voie 12",
        )
        .unwrap();
        let ancestor_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        repo.create_branch("etude-capacite", &ancestor_id).unwrap();

        repo.commit_change(
            "etude-capacite",
            GraphChange::new().add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12 (étude)")),
            "loic",
            "Renomme voie 12 (étude)",
        )
        .unwrap();
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12 (référence)")),
            "loic",
            "Renomme voie 12 (référence)",
        )
        .unwrap();

        // Premier essai sans résolution : conflit, comme attendu.
        assert!(matches!(
            repo.merge("etude-capacite", DEFAULT_BRANCH, "loic", "Merge"),
            Err(MergeError::Conflicts(_))
        ));

        // On choisit de garder la version de la source.
        let resolutions = vec![ConflictResolution::Modification {
            node_id: "voie-12".to_string(),
            keep: ResolutionSide::Source,
        }];
        let merge_id = repo
            .merge_resolve(
                "etude-capacite",
                DEFAULT_BRANCH,
                "loic",
                "Merge résolu",
                &resolutions,
                DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS,
            )
            .unwrap();

        let merged = repo.commit(&merge_id).unwrap();
        assert_eq!(merged.graph.nodes["voie-12"].label, "Voie 12 (étude)");
    }

    #[test]
    fn merge_resolve_applies_chosen_side_for_a_spatial_conflict() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("TA0", NodeKind::Voie, "Voie A")),
            "loic",
            "Ajout voie TA0",
        )
        .unwrap();
        let ancestor_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        repo.create_branch("etude-capacite", &ancestor_id).unwrap();

        repo.commit_change(
            "etude-capacite",
            GraphChange::new().add_node(point_node("aiguille-a", "TA0", 100.0)),
            "loic",
            "Ajout aiguille A (étude)",
        )
        .unwrap();
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(point_node("aiguille-b", "TA0", 104.0)),
            "loic",
            "Ajout aiguille B (référence)",
        )
        .unwrap();

        // On choisit de garder l'aiguillage de la cible (référence).
        let resolutions = vec![ConflictResolution::Spatial {
            source_node_id: "aiguille-a".to_string(),
            target_node_id: "aiguille-b".to_string(),
            keep: ResolutionSide::Target,
        }];
        let merge_id = repo
            .merge_resolve(
                "etude-capacite",
                DEFAULT_BRANCH,
                "loic",
                "Merge résolu",
                &resolutions,
                DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS,
            )
            .unwrap();

        let merged = repo.commit(&merge_id).unwrap();
        assert!(merged.graph.nodes.contains_key("aiguille-b"));
        assert!(!merged.graph.nodes.contains_key("aiguille-a"));
    }

    #[test]
    fn merge_resolve_reports_conflicts_still_missing_a_resolution() {
        let mut repo = Repository::init("loic", "Initialisation du dépôt");
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12")),
            "loic",
            "Ajout voie 12",
        )
        .unwrap();
        let ancestor_id = repo.branch_tip(DEFAULT_BRANCH).unwrap().clone();
        repo.create_branch("etude-capacite", &ancestor_id).unwrap();

        repo.commit_change(
            "etude-capacite",
            GraphChange::new().add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12 (étude)")),
            "loic",
            "Renomme voie 12 (étude)",
        )
        .unwrap();
        repo.commit_change(
            DEFAULT_BRANCH,
            GraphChange::new().add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12 (référence)")),
            "loic",
            "Renomme voie 12 (référence)",
        )
        .unwrap();

        // Aucune résolution fournie pour l'unique conflit : doit échouer
        // en listant précisément ce qui manque.
        let result = repo.merge_resolve(
            "etude-capacite",
            DEFAULT_BRANCH,
            "loic",
            "Merge résolu",
            &[],
            DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS,
        );
        match result {
            Err(MergeError::Conflicts(conflicts)) => assert_eq!(conflicts.len(), 1),
            other => panic!("attendu des conflits non résolus, obtenu : {other:?}"),
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
