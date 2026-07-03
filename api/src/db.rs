//! Persistance SQLite de CONTINUUM.
//!
//! Chaque commit stocke un instantané complet du graphe (cohérent avec le
//! modèle en mémoire de `graph-engine`), et l'historique (commit -> parent
//! -> parent...) est représenté par une table d'adjacence `commit_parents`,
//! comme Git le fait réellement en interne. Voir `migrations/0001_init.sql`
//! pour le schéma complet.
//!
//! Ce module réutilise les types de `continuum_graph_engine`
//! (`VersionedGraph`, `RailNode`, `Edge`) mais ne réutilise pas
//! `graph-engine::Repository` (qui reste en mémoire, pour `cli` et ses
//! propres tests) : ici, la base de données joue le rôle que le
//! `HashMap<String, CommitId>` jouait en mémoire.

use chrono::Utc;
use continuum_graph_engine::{
    compute_merge, resolve_merge, ConflictResolution, Edge, EdgeKind, GraphChange, MergeConflict,
    NodeKind, RailNode, VersionedGraph,
};
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Nom de la branche de référence — "référence" (avec l'accent) car c'est
/// le nom attendu par défaut côté front (`web/src/App.tsx`). Réutilisé
/// aussi comme point de départ par défaut pour créer une nouvelle branche.
pub const REFERENCE_BRANCH: &str = "référence";
const DEMO_STUDY_BRANCH: &str = "etude-capacite";
/// Branche recréée par le raccourci de démonstration `seed_demo_conflict`
/// (endpoint de développement `POST /debug/seed-conflict`).
const DEMO_CONFLICT_BRANCH: &str = "demo-conflit";

/// Erreurs métier de la couche persistance — distinctes des erreurs SQL
/// brutes, pour que `routes.rs` puisse les traduire en codes HTTP précis.
#[derive(Debug)]
pub enum DbError {
    BranchNotFound(String),
    BranchAlreadyExists(String),
    CommitNotFound(String),
    NoCommonAncestor,
    /// Le scénario de démonstration (`seed_demo_conflict`) n'a pas pu être
    /// construit à partir de l'état actuel de la branche référence.
    DemoConflictUnavailable(String),
    Sql(sqlx::Error),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::BranchNotFound(name) => write!(f, "branche inconnue : {name}"),
            DbError::BranchAlreadyExists(name) => write!(f, "la branche existe déjà : {name}"),
            DbError::CommitNotFound(id) => write!(f, "commit inconnu : {id}"),
            DbError::NoCommonAncestor => write!(f, "aucun ancêtre commun trouvé"),
            DbError::DemoConflictUnavailable(reason) => {
                write!(f, "scénario de démonstration impossible : {reason}")
            }
            DbError::Sql(err) => write!(f, "erreur base de données : {err}"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<sqlx::Error> for DbError {
    fn from(err: sqlx::Error) -> Self {
        DbError::Sql(err)
    }
}

/// Ouvre (et crée si besoin) le fichier SQLite, puis applique les
/// migrations versionnées de `api/migrations/`. Idempotent : rejouer les
/// migrations sur une base déjà à jour ne fait rien.
pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new().connect_with(options).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

fn node_kind_to_text(kind: &NodeKind) -> String {
    serde_json::to_string(kind).expect("NodeKind se sérialise toujours en JSON")
}

fn node_kind_from_text(text: &str) -> NodeKind {
    serde_json::from_str(text).expect("valeur de kind invalide en base")
}

fn edge_kind_to_text(kind: &EdgeKind) -> String {
    serde_json::to_string(kind).expect("EdgeKind se sérialise toujours en JSON")
}

fn edge_kind_from_text(text: &str) -> EdgeKind {
    serde_json::from_str(text).expect("valeur de kind invalide en base")
}

/// Insère un nouveau commit (avec l'instantané complet de `graph`) et ses
/// liens de parenté. Renvoie l'id généré (UUID v4).
///
/// Les trois insertions (commit, parents, contenu du graphe) sont
/// regroupées dans une transaction : soit tout est écrit, soit rien ne
/// l'est en cas d'erreur.
pub async fn insert_commit(
    pool: &SqlitePool,
    parents: &[String],
    author: &str,
    message: &str,
    graph: &VersionedGraph,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    let mut tx = pool.begin().await?;

    sqlx::query("INSERT INTO commits (id, author, message, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(author)
        .bind(message)
        .bind(&created_at)
        .execute(&mut *tx)
        .await?;

    for (order, parent_id) in parents.iter().enumerate() {
        sqlx::query(
            "INSERT INTO commit_parents (commit_id, parent_id, parent_order) VALUES (?, ?, ?)",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(order as i64)
        .execute(&mut *tx)
        .await?;
    }

    for node in graph.nodes.values() {
        let properties = serde_json::to_string(&node.properties)
            .expect("les propriétés d'un nœud se sérialisent toujours en JSON");
        sqlx::query(
            "INSERT INTO commit_nodes (commit_id, node_id, kind, label, properties) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&node.id)
        .bind(node_kind_to_text(&node.kind))
        .bind(&node.label)
        .bind(properties)
        .execute(&mut *tx)
        .await?;
    }

    for edge in &graph.edges {
        sqlx::query(
            "INSERT INTO commit_edges (commit_id, from_node, to_node, kind) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&edge.from)
        .bind(&edge.to)
        .bind(edge_kind_to_text(&edge.kind))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(id)
}

/// Fait pointer une branche vers un commit (la crée si elle n'existe pas
/// encore, la déplace sinon) — équivalent de
/// `branches.insert(name, commit_id)` sur le `HashMap` en mémoire.
pub async fn set_branch(pool: &SqlitePool, name: &str, commit_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO branches (name, commit_id) VALUES (?, ?) \
         ON CONFLICT(name) DO UPDATE SET commit_id = excluded.commit_id",
    )
    .bind(name)
    .bind(commit_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Le commit actuellement pointé par une branche, si elle existe.
pub async fn branch_tip(pool: &SqlitePool, name: &str) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query("SELECT commit_id FROM branches WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get::<String, _>("commit_id")))
}

/// La liste des noms de branches existantes.
pub async fn list_branch_names(pool: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SELECT name FROM branches ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.get::<String, _>("name")).collect())
}

/// Reconstruit l'état complet du graphe à un commit donné (« time
/// machine ») : une simple lecture, puisque chaque commit stocke déjà un
/// instantané complet.
pub async fn graph_at(pool: &SqlitePool, commit_id: &str) -> Result<VersionedGraph, sqlx::Error> {
    let mut graph = VersionedGraph::new();

    let node_rows =
        sqlx::query("SELECT node_id, kind, label, properties FROM commit_nodes WHERE commit_id = ?")
            .bind(commit_id)
            .fetch_all(pool)
            .await?;
    for row in node_rows {
        let node_id: String = row.get("node_id");
        let kind_text: String = row.get("kind");
        let label: String = row.get("label");
        let properties_text: String = row.get("properties");
        let mut node = RailNode::new(node_id, node_kind_from_text(&kind_text), label);
        node.properties = serde_json::from_str(&properties_text).unwrap_or_default();
        graph.add_node(node);
    }

    let edge_rows =
        sqlx::query("SELECT from_node, to_node, kind FROM commit_edges WHERE commit_id = ?")
            .bind(commit_id)
            .fetch_all(pool)
            .await?;
    for row in edge_rows {
        let from: String = row.get("from_node");
        let to: String = row.get("to_node");
        let kind_text: String = row.get("kind");
        graph.add_edge(Edge {
            from,
            to,
            kind: edge_kind_from_text(&kind_text),
        });
    }

    Ok(graph)
}

/// Le graphe pointé par une branche, ou `None` si la branche n'existe pas.
pub async fn branch_graph(
    pool: &SqlitePool,
    name: &str,
) -> Result<Option<VersionedGraph>, sqlx::Error> {
    match branch_tip(pool, name).await? {
        Some(commit_id) => Ok(Some(graph_at(pool, &commit_id).await?)),
        None => Ok(None),
    }
}

/// Vrai si un commit avec cet id existe.
pub async fn commit_exists(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT 1 FROM commits WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

/// Crée une nouvelle branche pointant sur un commit existant — ne duplique
/// rien, comme `Repository::create_branch` en mémoire (étape 1).
pub async fn create_branch(
    pool: &SqlitePool,
    name: &str,
    from_commit: &str,
) -> Result<(), DbError> {
    if branch_tip(pool, name).await?.is_some() {
        return Err(DbError::BranchAlreadyExists(name.to_string()));
    }
    if !commit_exists(pool, from_commit).await? {
        return Err(DbError::CommitNotFound(from_commit.to_string()));
    }
    set_branch(pool, name, from_commit).await?;
    Ok(())
}

/// Committer un changement (ajout/suppression de nœuds, pas encore
/// d'arêtes ni de modification de propriétés à cette étape) sur une
/// branche : charge l'état courant, applique le changement, crée un
/// nouveau commit dont le parent est l'ancien tip de la branche.
pub async fn commit_change(
    pool: &SqlitePool,
    branch: &str,
    change: &GraphChange,
    author: &str,
    message: &str,
) -> Result<String, DbError> {
    let parent_id = branch_tip(pool, branch)
        .await?
        .ok_or_else(|| DbError::BranchNotFound(branch.to_string()))?;

    let mut graph = graph_at(pool, &parent_id).await?;
    graph.apply(change);

    let new_id = insert_commit(pool, &[parent_id], author, message, &graph).await?;
    set_branch(pool, branch, &new_id).await?;
    Ok(new_id)
}

/// Résumé d'un commit tel qu'exposé par `GET /branches/{name}/history` —
/// pas besoin du graphe complet ici, juste ses métadonnées et ses parents.
#[derive(Debug, Serialize)]
pub struct CommitSummary {
    pub id: String,
    pub parents: Vec<String>,
    pub author: String,
    pub message: String,
    pub created_at: String,
}

async fn commit_summary(pool: &SqlitePool, id: &str) -> Result<CommitSummary, sqlx::Error> {
    let row = sqlx::query("SELECT id, author, message, created_at FROM commits WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let parent_rows =
        sqlx::query("SELECT parent_id FROM commit_parents WHERE commit_id = ? ORDER BY parent_order")
            .bind(id)
            .fetch_all(pool)
            .await?;
    Ok(CommitSummary {
        id: row.get("id"),
        parents: parent_rows
            .into_iter()
            .map(|r| r.get::<String, _>("parent_id"))
            .collect(),
        author: row.get("author"),
        message: row.get("message"),
        created_at: row.get("created_at"),
    })
}

/// L'historique d'une branche, du commit le plus récent au plus ancien —
/// on ne suit que le premier parent (comme `git log` par défaut), en
/// s'arrêtant au commit racine (sans parent). `None` si la branche
/// n'existe pas.
pub async fn branch_history(
    pool: &SqlitePool,
    branch: &str,
) -> Result<Option<Vec<CommitSummary>>, sqlx::Error> {
    let Some(mut current) = branch_tip(pool, branch).await? else {
        return Ok(None);
    };

    let mut history = Vec::new();
    loop {
        let summary = commit_summary(pool, &current).await?;
        let next = summary.parents.first().cloned();
        history.push(summary);
        match next {
            Some(parent_id) => current = parent_id,
            None => break,
        }
    }
    Ok(Some(history))
}

/// Profondeur de tous les ancêtres de `start` (lui-même inclus, profondeur
/// 0), en remontant tous les parents (un commit de fusion en a deux).
/// Port SQL exact de `Repository::ancestor_depths` (étape 1, en mémoire).
async fn ancestor_depths(pool: &SqlitePool, start: &str) -> Result<HashMap<String, u32>, sqlx::Error> {
    let mut depths: HashMap<String, u32> = HashMap::new();
    let mut frontier = vec![(start.to_string(), 0u32)];
    while let Some((id, depth)) = frontier.pop() {
        let already_better = matches!(depths.get(&id), Some(&existing) if existing <= depth);
        if already_better {
            continue;
        }
        depths.insert(id.clone(), depth);

        let parent_rows = sqlx::query("SELECT parent_id FROM commit_parents WHERE commit_id = ?")
            .bind(&id)
            .fetch_all(pool)
            .await?;
        for row in parent_rows {
            let parent_id: String = row.get("parent_id");
            frontier.push((parent_id, depth + 1));
        }
    }
    Ok(depths)
}

/// Ancêtre commun le plus proche de deux commits (approximation du
/// "merge-base" de Git) — port SQL exact de
/// `Repository::find_common_ancestor` (étape 1, en mémoire).
async fn find_common_ancestor(
    pool: &SqlitePool,
    a: &str,
    b: &str,
) -> Result<Option<String>, sqlx::Error> {
    let depths_a = ancestor_depths(pool, a).await?;
    let depths_b = ancestor_depths(pool, b).await?;
    Ok(depths_a
        .iter()
        .filter_map(|(id, &da)| depths_b.get(id).map(|&db| (id.clone(), da + db)))
        .min_by_key(|(_, total)| *total)
        .map(|(id, _)| id))
}

/// Résultat d'une tentative de fusion.
pub enum MergeOutcome {
    Merged(String),
    Conflicts(Vec<MergeConflict>),
}

/// Résout les tips des deux branches, leur ancêtre commun, et charge les 3
/// instantanés de graphe correspondants — la partie commune à
/// `merge_branches_with_threshold` et `merge_resolve_with_threshold`.
async fn load_merge_graphs(
    pool: &SqlitePool,
    source: &str,
    target: &str,
) -> Result<(String, String, VersionedGraph, VersionedGraph, VersionedGraph), DbError> {
    let source_tip = branch_tip(pool, source)
        .await?
        .ok_or_else(|| DbError::BranchNotFound(source.to_string()))?;
    let target_tip = branch_tip(pool, target)
        .await?
        .ok_or_else(|| DbError::BranchNotFound(target.to_string()))?;

    let ancestor_id = find_common_ancestor(pool, &source_tip, &target_tip)
        .await?
        .ok_or(DbError::NoCommonAncestor)?;

    let ancestor_graph = graph_at(pool, &ancestor_id).await?;
    let source_graph = graph_at(pool, &source_tip).await?;
    let target_graph = graph_at(pool, &target_tip).await?;

    Ok((target_tip, source_tip, ancestor_graph, source_graph, target_graph))
}

/// Fusionne `source` dans `target` : ancêtre commun, puis délègue le calcul
/// (conflits ou graphe fusionné) à `compute_merge` — la même fonction pure
/// utilisée par `Repository::merge` en mémoire (étape 1), pour ne pas
/// dupliquer l'algorithme de détection de conflit entre les deux
/// implémentations. Cette fonction ne fait que le travail spécifique à SQL :
/// charger les 3 graphes, puis écrire le commit de fusion s'il n'y a pas
/// de conflit.
pub async fn merge_branches_with_threshold(
    pool: &SqlitePool,
    source: &str,
    target: &str,
    author: &str,
    message: &str,
    spatial_threshold_meters: f64,
) -> Result<MergeOutcome, DbError> {
    let (target_tip, source_tip, ancestor_graph, source_graph, target_graph) =
        load_merge_graphs(pool, source, target).await?;

    let merged = match compute_merge(
        &ancestor_graph,
        &source_graph,
        &target_graph,
        spatial_threshold_meters,
    ) {
        Ok(merged) => merged,
        Err(conflicts) => return Ok(MergeOutcome::Conflicts(conflicts)),
    };

    let commit_id = insert_commit(pool, &[target_tip, source_tip], author, message, &merged).await?;
    set_branch(pool, target, &commit_id).await?;
    Ok(MergeOutcome::Merged(commit_id))
}

/// Fusionne `source` dans `target` en appliquant des résolutions choisies
/// par l'utilisateur pour chaque conflit — voir
/// `graph_engine::resolve_merge` (les conflits sont recalculés à partir de
/// l'état courant des branches, pas d'une liste transmise par le client).
pub async fn merge_resolve_with_threshold(
    pool: &SqlitePool,
    source: &str,
    target: &str,
    author: &str,
    message: &str,
    resolutions: &[ConflictResolution],
    spatial_threshold_meters: f64,
) -> Result<MergeOutcome, DbError> {
    let (target_tip, source_tip, ancestor_graph, source_graph, target_graph) =
        load_merge_graphs(pool, source, target).await?;

    let merged = match resolve_merge(
        &ancestor_graph,
        &source_graph,
        &target_graph,
        resolutions,
        spatial_threshold_meters,
    ) {
        Ok(merged) => merged,
        Err(conflicts) => return Ok(MergeOutcome::Conflicts(conflicts)),
    };

    let commit_id = insert_commit(pool, &[target_tip, source_tip], author, message, &merged).await?;
    set_branch(pool, target, &commit_id).await?;
    Ok(MergeOutcome::Merged(commit_id))
}

/// Le vrai jeu de données de démonstration officiel d'OSRD ("small_infra"),
/// embarqué dans le binaire à la compilation — voir `examples/small_infra.json`
/// à la racine du dépôt (même fichier que celui utilisé comme fixture de
/// test dans `osrd-bridge/tests/`).
const SMALL_INFRA_RAILJSON: &str = include_str!("../../examples/small_infra.json");

/// Si la base est vide (aucune branche), importe la vraie infrastructure
/// OSRD "small_infra" (voies, aiguillages, signaux réels) comme premier
/// commit de la branche de référence, puis crée "etude-capacite" par
/// dessus avec un ajout illustratif — pour que la démo fonctionne
/// immédiatement après un `git clone` frais, avec un jeu de données
/// réaliste plutôt que deux nœuds jouets.
pub async fn seed_if_empty(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    if !list_branch_names(pool).await?.is_empty() {
        return Ok(());
    }

    let reference = continuum_osrd_bridge::import_from_railjson(SMALL_INFRA_RAILJSON)?;
    let reference_commit = insert_commit(
        pool,
        &[],
        "system",
        "Initialisation : infrastructure réelle OSRD « small_infra » (référence)",
        &reference,
    )
    .await?;
    set_branch(pool, REFERENCE_BRANCH, &reference_commit).await?;

    let mut etude_capacite = reference.clone();
    etude_capacite.add_node(RailNode::new(
        "aiguille-45",
        NodeKind::AppareilDeVoie,
        "Aiguille 45 (hypothèse d'ajout - étude de capacité)",
    ));
    etude_capacite.add_edge(Edge {
        from: "aiguille-45".into(),
        to: "TA0".into(),
        kind: EdgeKind::Appartenance,
    });
    let etude_commit = insert_commit(
        pool,
        &[reference_commit],
        "system",
        "Hypothèse : ajout aiguille 45 (étude de capacité)",
        &etude_capacite,
    )
    .await?;
    set_branch(pool, DEMO_STUDY_BRANCH, &etude_commit).await?;

    Ok(())
}

/// Génère en une seule fois un conflit spatial de démonstration réaliste :
/// prend deux aiguillages existants de la branche référence et les
/// déplace à quelques mètres l'un de l'autre sur la même voie, chacun
/// dans une branche différente (`référence` et `demo-conflit`).
///
/// **Endpoint de développement, pas destiné à un usage en production** :
/// un raccourci pour tester/démontrer la détection de conflit spatial
/// sans construire le scénario à la main via l'API — pas une
/// fonctionnalité finale du produit.
///
/// Si `demo-conflit` existe déjà, elle est simplement repointée sur un
/// nouveau point de départ (pas de suppression de branche dans ce
/// modèle) : la façon la plus simple de "recréer proprement".
pub async fn seed_demo_conflict(pool: &SqlitePool) -> Result<(String, String), DbError> {
    let reference_tip = branch_tip(pool, REFERENCE_BRANCH)
        .await?
        .ok_or_else(|| DbError::BranchNotFound(REFERENCE_BRANCH.to_string()))?;
    let graph = graph_at(pool, &reference_tip).await?;

    let mut switch_ids: Vec<&String> = graph
        .nodes
        .values()
        .filter(|n| n.kind == NodeKind::AppareilDeVoie)
        .map(|n| &n.id)
        .collect();
    switch_ids.sort();
    let (switch_a_id, switch_b_id) = match (switch_ids.first(), switch_ids.get(1)) {
        (Some(a), Some(b)) => ((*a).clone(), (*b).clone()),
        _ => {
            return Err(DbError::DemoConflictUnavailable(
                "il faut au moins deux aiguillages dans la branche référence".to_string(),
            ))
        }
    };

    let mut track_ids: Vec<&String> = graph
        .nodes
        .values()
        .filter(|n| n.kind == NodeKind::Voie)
        .map(|n| &n.id)
        .collect();
    track_ids.sort();
    let track_id = track_ids
        .first()
        .ok_or_else(|| {
            DbError::DemoConflictUnavailable("aucune voie dans la branche référence".to_string())
        })?
        .to_string();

    // Nouvelle branche de démonstration à partir du tip courant de
    // référence, avant que les deux côtés ne divergent.
    set_branch(pool, DEMO_CONFLICT_BRANCH, &reference_tip).await?;

    // La nouvelle position est calculée à partir de la position *actuelle*
    // de l'aiguillage (+1m), pas une valeur figée : si ce raccourci a déjà
    // été utilisé une fois, réutiliser une constante produirait un
    // changement identique à l'ancêtre commun (donc aucun diff détecté, et
    // pas de conflit). Ainsi, cliquer plusieurs fois de suite produit à
    // chaque fois un vrai conflit, en avançant un peu plus loin sur la voie.
    let current_position_a = graph
        .nodes
        .get(&switch_a_id)
        .and_then(|n| n.properties.get("position"))
        .and_then(|v| v.as_f64())
        .unwrap_or(50.0);
    let new_position_a = current_position_a + 1.0;
    let new_position_b = new_position_a + 4.0; // 4m plus loin : sous le seuil par défaut (8m)

    let mut node_a = graph
        .nodes
        .get(&switch_a_id)
        .cloned()
        .expect("switch_a_id vient de graph.nodes");
    node_a
        .properties
        .insert("track".to_string(), serde_json::json!(track_id));
    node_a
        .properties
        .insert("position".to_string(), serde_json::json!(new_position_a));
    commit_change(
        pool,
        REFERENCE_BRANCH,
        &GraphChange::new().add_node(node_a),
        "system",
        &format!("Démo : déplace {switch_a_id} sur {track_id} (position {new_position_a}m)"),
    )
    .await?;

    let mut node_b = graph
        .nodes
        .get(&switch_b_id)
        .cloned()
        .expect("switch_b_id vient de graph.nodes");
    node_b
        .properties
        .insert("track".to_string(), serde_json::json!(track_id));
    node_b
        .properties
        .insert("position".to_string(), serde_json::json!(new_position_b));
    commit_change(
        pool,
        DEMO_CONFLICT_BRANCH,
        &GraphChange::new().add_node(node_b),
        "system",
        &format!("Démo : déplace {switch_b_id} sur {track_id} (position {new_position_b}m)"),
    )
    .await?;

    Ok((REFERENCE_BRANCH.to_string(), DEMO_CONFLICT_BRANCH.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un fichier SQLite temporaire réel (pas `:memory:`, qui ne survivrait
    /// pas à la fermeture de la connexion) pour simuler un redémarrage.
    async fn temp_db_url() -> (SqlitePool, tempfile::TempPath) {
        let file = tempfile::NamedTempFile::new().expect("création du fichier temporaire");
        let path = file.into_temp_path();
        let url = format!("sqlite://{}", path.display());
        let pool = connect(&url).await.expect("connexion à la base temporaire");
        (pool, path)
    }

    #[tokio::test]
    async fn commit_survives_a_simulated_restart() {
        let (pool, path) = temp_db_url().await;

        let mut graph = VersionedGraph::new();
        graph.add_node(RailNode::new("voie-12", NodeKind::Voie, "Voie 12"));
        let commit_id = insert_commit(&pool, &[], "loic", "premier commit", &graph)
            .await
            .unwrap();
        set_branch(&pool, "reference", &commit_id).await.unwrap();

        // Ferme la connexion : simule l'arrêt du serveur.
        pool.close().await;

        // Rouvre une nouvelle connexion vers le même fichier : simule un
        // redémarrage du serveur après un `cargo run` ultérieur.
        let url = format!("sqlite://{}", path.display());
        let reopened = connect(&url).await.unwrap();

        let reloaded_tip = branch_tip(&reopened, "reference").await.unwrap().unwrap();
        assert_eq!(reloaded_tip, commit_id);

        let reloaded_graph = graph_at(&reopened, &reloaded_tip).await.unwrap();
        assert!(reloaded_graph.nodes.contains_key("voie-12"));

        reopened.close().await;
    }

    #[tokio::test]
    async fn seed_if_empty_creates_demo_branches_once() {
        let (pool, _path) = temp_db_url().await;

        seed_if_empty(&pool).await.unwrap();
        let names = list_branch_names(&pool).await.unwrap();
        assert!(names.contains(&REFERENCE_BRANCH.to_string()));
        assert!(names.contains(&DEMO_STUDY_BRANCH.to_string()));

        let reference_graph = branch_graph(&pool, REFERENCE_BRANCH).await.unwrap().unwrap();
        // 31 voies + 17 aiguillages + 106 signaux dans le vrai small_infra.
        assert_eq!(reference_graph.nodes.len(), 31 + 17 + 106);
        assert!(reference_graph.nodes.contains_key("TA0"));

        let etude_graph = branch_graph(&pool, DEMO_STUDY_BRANCH)
            .await
            .unwrap()
            .unwrap();
        assert!(etude_graph.nodes.contains_key("TA0"));
        assert!(etude_graph.nodes.contains_key("aiguille-45"));

        // Un second appel ne doit rien dupliquer.
        seed_if_empty(&pool).await.unwrap();
        let names_again = list_branch_names(&pool).await.unwrap();
        assert_eq!(names.len(), names_again.len());

        pool.close().await;
    }
}
