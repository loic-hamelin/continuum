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
use continuum_graph_engine::{Edge, EdgeKind, NodeKind, RailNode, VersionedGraph};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;

/// Nom des deux branches de démonstration recréées au premier démarrage.
/// "référence" (avec l'accent) car c'est le nom attendu par défaut côté
/// front (`web/src/App.tsx`).
const DEMO_REFERENCE_BRANCH: &str = "référence";
const DEMO_STUDY_BRANCH: &str = "etude-capacite";

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

/// Si la base est vide (aucune branche), recrée les deux branches de
/// démonstration comme deux commits liés par une relation parent -> enfant
/// ("etude-capacite" descend de "référence"), pour que la démo fonctionne
/// immédiatement après un `git clone` frais.
pub async fn seed_if_empty(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    if !list_branch_names(pool).await?.is_empty() {
        return Ok(());
    }

    let mut reference = VersionedGraph::new();
    reference.add_node(RailNode::new(
        "voie-12",
        NodeKind::Voie,
        "Voie 12 - gare de Lens",
    ));
    let reference_commit = insert_commit(
        pool,
        &[],
        "system",
        "Initialisation : état de référence",
        &reference,
    )
    .await?;
    set_branch(pool, DEMO_REFERENCE_BRANCH, &reference_commit).await?;

    let mut etude_capacite = reference.clone();
    etude_capacite.add_node(RailNode::new(
        "aiguille-45",
        NodeKind::AppareilDeVoie,
        "Aiguille 45 (hypothèse d'ajout)",
    ));
    etude_capacite.add_edge(Edge {
        from: "aiguille-45".into(),
        to: "voie-12".into(),
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
        assert!(names.contains(&DEMO_REFERENCE_BRANCH.to_string()));
        assert!(names.contains(&DEMO_STUDY_BRANCH.to_string()));

        let etude_graph = branch_graph(&pool, DEMO_STUDY_BRANCH)
            .await
            .unwrap()
            .unwrap();
        assert!(etude_graph.nodes.contains_key("voie-12"));
        assert!(etude_graph.nodes.contains_key("aiguille-45"));

        // Un second appel ne doit rien dupliquer.
        seed_if_empty(&pool).await.unwrap();
        let names_again = list_branch_names(&pool).await.unwrap();
        assert_eq!(names.len(), names_again.len());

        pool.close().await;
    }
}
