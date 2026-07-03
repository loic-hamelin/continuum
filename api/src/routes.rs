//! Handlers HTTP de l'API CONTINUUM.
//!
//! De fines fonctions actix-web qui appellent `db.rs` et traduisent ses
//! erreurs (`DbError`) en réponses HTTP — pas de logique métier ici.

use crate::db::{self, DbError};
use crate::AppState;
use actix_web::{get, post, web, HttpResponse, Responder};
use continuum_graph_engine::{ConflictResolution, GraphChange, RailNode};
use serde::Deserialize;
use std::collections::HashMap;

/// Traduit une erreur métier de la couche persistance en réponse HTTP,
/// avec le même style que le reste de l'API (`serde_json::json!`).
fn db_error_response(err: DbError) -> HttpResponse {
    match err {
        DbError::BranchNotFound(name) => HttpResponse::NotFound()
            .json(serde_json::json!({ "error": format!("branche inconnue : {name}") })),
        DbError::BranchAlreadyExists(name) => HttpResponse::Conflict()
            .json(serde_json::json!({ "error": format!("la branche existe déjà : {name}") })),
        DbError::CommitNotFound(id) => HttpResponse::NotFound()
            .json(serde_json::json!({ "error": format!("commit inconnu : {id}") })),
        DbError::NoCommonAncestor => HttpResponse::Conflict()
            .json(serde_json::json!({ "error": "aucun ancêtre commun trouvé" })),
        DbError::DemoConflictUnavailable(reason) => HttpResponse::UnprocessableEntity()
            .json(serde_json::json!({ "error": format!("scénario de démonstration impossible : {reason}") })),
        DbError::Sql(err) => {
            HttpResponse::InternalServerError().json(serde_json::json!({ "error": err.to_string() }))
        }
    }
}

/// GET /health — vérifie que l'API répond.
#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

/// GET /api-docs/openapi.json — sert la spécification OpenAPI de l'API,
/// sur le modèle de ce qu'expose editoast dans OSRD.
#[get("/api-docs/openapi.json")]
async fn openapi_spec() -> impl Responder {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(include_str!("../openapi/openapi.json"))
}

/// GET /branches — liste les branches disponibles.
#[get("/branches")]
async fn list_branches(state: web::Data<AppState>) -> impl Responder {
    match db::list_branch_names(&state.pool).await {
        Ok(names) => HttpResponse::Ok().json(names),
        Err(err) => {
            HttpResponse::InternalServerError().json(serde_json::json!({ "error": err.to_string() }))
        }
    }
}

#[derive(Deserialize)]
struct CreateBranchRequest {
    name: String,
    from_commit: Option<String>,
}

/// POST /branches — crée une nouvelle branche à partir d'un commit
/// existant (par défaut, le dernier commit de la branche de référence).
#[post("/branches")]
async fn create_branch(
    state: web::Data<AppState>,
    body: web::Json<CreateBranchRequest>,
) -> impl Responder {
    let from_commit = match &body.from_commit {
        Some(id) => id.clone(),
        None => match db::branch_tip(&state.pool, db::REFERENCE_BRANCH).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                return HttpResponse::NotFound().json(
                    serde_json::json!({ "error": "branche de référence introuvable" }),
                )
            }
            Err(err) => {
                return HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "error": err.to_string() }))
            }
        },
    };

    match db::create_branch(&state.pool, &body.name, &from_commit).await {
        Ok(()) => {
            HttpResponse::Created().json(serde_json::json!({ "name": body.name, "commit_id": from_commit }))
        }
        Err(err) => db_error_response(err),
    }
}

/// GET /branches/{name} — renvoie le graphe complet d'une branche.
#[get("/branches/{name}")]
async fn get_branch(state: web::Data<AppState>, name: web::Path<String>) -> impl Responder {
    match db::branch_graph(&state.pool, &name).await {
        Ok(Some(graph)) => HttpResponse::Ok().json(graph),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({ "error": "branche inconnue" })),
        Err(err) => {
            HttpResponse::InternalServerError().json(serde_json::json!({ "error": err.to_string() }))
        }
    }
}

#[derive(Deserialize)]
struct CommitChangeRequest {
    author: String,
    message: String,
    #[serde(default)]
    add_nodes: Vec<RailNode>,
    #[serde(default)]
    remove_nodes: Vec<String>,
}

/// POST /branches/{name}/commits — committe un changement (ajout/
/// suppression de nœuds) sur cette branche.
#[post("/branches/{name}/commits")]
async fn commit_on_branch(
    state: web::Data<AppState>,
    name: web::Path<String>,
    body: web::Json<CommitChangeRequest>,
) -> impl Responder {
    let mut change = GraphChange::new();
    for node in body.add_nodes.clone() {
        change = change.add_node(node);
    }
    for id in body.remove_nodes.clone() {
        change = change.remove_node(id);
    }

    match db::commit_change(&state.pool, &name, &change, &body.author, &body.message).await {
        Ok(commit_id) => HttpResponse::Created().json(serde_json::json!({ "commit_id": commit_id })),
        Err(err) => db_error_response(err),
    }
}

/// GET /branches/{name}/history — l'historique de la branche, du commit
/// le plus récent au plus ancien.
#[get("/branches/{name}/history")]
async fn branch_history(state: web::Data<AppState>, name: web::Path<String>) -> impl Responder {
    match db::branch_history(&state.pool, &name).await {
        Ok(Some(history)) => HttpResponse::Ok().json(history),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({ "error": "branche inconnue" })),
        Err(err) => {
            HttpResponse::InternalServerError().json(serde_json::json!({ "error": err.to_string() }))
        }
    }
}

/// GET /commits/{id}/graph — reconstruit l'état complet du graphe à ce
/// commit précis (la « time machine » de l'étape 1, exposée ici).
#[get("/commits/{id}/graph")]
async fn commit_graph(state: web::Data<AppState>, id: web::Path<String>) -> impl Responder {
    match db::commit_exists(&state.pool, &id).await {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::NotFound()
                .json(serde_json::json!({ "error": format!("commit inconnu : {}", id.as_str()) }))
        }
        Err(err) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": err.to_string() }))
        }
    }

    match db::graph_at(&state.pool, &id).await {
        Ok(graph) => HttpResponse::Ok().json(graph),
        Err(err) => {
            HttpResponse::InternalServerError().json(serde_json::json!({ "error": err.to_string() }))
        }
    }
}

/// GET /diff?base=...&compare=... — compare deux branches.
#[get("/diff")]
async fn diff_branches(
    state: web::Data<AppState>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    let base_name = query.get("base").cloned().unwrap_or_default();
    let compare_name = query.get("compare").cloned().unwrap_or_default();

    let base = match db::branch_graph(&state.pool, &base_name).await {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(serde_json::json!({ "error": "base ou compare introuvable" }))
        }
        Err(err) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": err.to_string() }))
        }
    };
    let compare = match db::branch_graph(&state.pool, &compare_name).await {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(serde_json::json!({ "error": "base ou compare introuvable" }))
        }
        Err(err) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({ "error": err.to_string() }))
        }
    };

    let result = base.diff(&compare);
    let added: Vec<&RailNode> = result.added;
    let removed: Vec<&RailNode> = result.removed;
    HttpResponse::Ok().json(serde_json::json!({ "added": added, "removed": removed }))
}

#[derive(Deserialize)]
struct MergeRequest {
    source: String,
    target: String,
    author: String,
    message: String,
    /// Distance (mètres) en-deçà de laquelle deux objets ponctuels
    /// différents sur la même voie sont en conflit spatial — optionnel,
    /// défaut `DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS` (8m).
    spatial_threshold_meters: Option<f64>,
}

/// POST /merge — tente une fusion entre une branche source et une branche
/// cible. Renvoie soit le commit de fusion créé, soit la liste structurée
/// des conflits (modification, suppression-vs-modification, spatial) si
/// la fusion échoue.
#[post("/merge")]
async fn merge_branches(state: web::Data<AppState>, body: web::Json<MergeRequest>) -> impl Responder {
    let threshold = body
        .spatial_threshold_meters
        .unwrap_or(continuum_graph_engine::DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS);

    match db::merge_branches_with_threshold(
        &state.pool,
        &body.source,
        &body.target,
        &body.author,
        &body.message,
        threshold,
    )
    .await
    {
        Ok(db::MergeOutcome::Merged(commit_id)) => {
            HttpResponse::Created().json(serde_json::json!({ "commit_id": commit_id }))
        }
        Ok(db::MergeOutcome::Conflicts(conflicts)) => HttpResponse::Conflict().json(serde_json::json!({
            "error": format!("{} conflit(s) détecté(s)", conflicts.len()),
            "conflicts": conflicts,
        })),
        Err(err) => db_error_response(err),
    }
}

#[derive(Deserialize)]
struct MergeResolveRequest {
    source: String,
    target: String,
    author: String,
    message: String,
    resolutions: Vec<ConflictResolution>,
    spatial_threshold_meters: Option<f64>,
}

/// POST /merge/resolve — applique des résolutions choisies par
/// l'utilisateur pour chaque conflit d'une fusion, et committe le
/// résultat. Action distincte de `POST /merge` (qui ne fait que tenter une
/// fusion et échouer proprement) : ici, on committe pour de bon une fois
/// toutes les résolutions choisies. Si des conflits restent non couverts
/// par `resolutions` (ou si les branches ont bougé depuis la
/// prévisualisation), renvoie la liste de ce qui manque encore.
#[post("/merge/resolve")]
async fn merge_resolve(
    state: web::Data<AppState>,
    body: web::Json<MergeResolveRequest>,
) -> impl Responder {
    let threshold = body
        .spatial_threshold_meters
        .unwrap_or(continuum_graph_engine::DEFAULT_SPATIAL_CONFLICT_THRESHOLD_METERS);

    match db::merge_resolve_with_threshold(
        &state.pool,
        &body.source,
        &body.target,
        &body.author,
        &body.message,
        &body.resolutions,
        threshold,
    )
    .await
    {
        Ok(db::MergeOutcome::Merged(commit_id)) => {
            HttpResponse::Created().json(serde_json::json!({ "commit_id": commit_id }))
        }
        Ok(db::MergeOutcome::Conflicts(conflicts)) => HttpResponse::Conflict().json(serde_json::json!({
            "error": format!("{} conflit(s) non résolu(s)", conflicts.len()),
            "conflicts": conflicts,
        })),
        Err(err) => db_error_response(err),
    }
}

/// POST /debug/seed-conflict — génère en une fois un conflit spatial de
/// démonstration réaliste (déplace deux aiguillages existants à quelques
/// mètres l'un de l'autre sur la même voie, dans deux branches
/// différentes) et renvoie les deux branches à comparer.
///
/// **Endpoint de développement, pas destiné à un usage en production** :
/// un raccourci pour tester/démontrer la détection de conflit spatial
/// sans construire le scénario à la main via l'API — pas une
/// fonctionnalité finale du produit.
#[post("/debug/seed-conflict")]
async fn seed_demo_conflict(state: web::Data<AppState>) -> impl Responder {
    match db::seed_demo_conflict(&state.pool).await {
        Ok((branch_a, branch_b)) => {
            HttpResponse::Ok().json(serde_json::json!({ "branch_a": branch_a, "branch_b": branch_b }))
        }
        Err(err) => db_error_response(err),
    }
}

/// Enregistre tous les endpoints de l'API sur l'application actix-web.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(health)
        .service(openapi_spec)
        .service(list_branches)
        .service(create_branch)
        .service(get_branch)
        .service(commit_on_branch)
        .service(branch_history)
        .service(commit_graph)
        .service(diff_branches)
        .service(merge_branches)
        .service(merge_resolve)
        .service(seed_demo_conflict);
}
