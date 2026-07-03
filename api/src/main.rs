//! CONTINUUM — api
//!
//! Service HTTP qui expose le graphe versionné via une API REST, sur le
//! modèle d'editoast dans OSRD (service Rust qui récupère les données et
//! les rend disponibles via des endpoints HTTP documentés).
//!
//! Les données sont persistées dans un fichier SQLite (voir `db.rs` et
//! `migrations/`) : l'historique des commits et des branches survit au
//! redémarrage du serveur.

mod db;

use actix_cors::Cors;
use actix_web::{get, web, App, HttpServer, HttpResponse, Responder};
use continuum_graph_engine::RailNode;
use sqlx::SqlitePool;
use std::collections::HashMap;

struct AppState {
    pool: SqlitePool,
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
    HttpResponse::Ok().content_type("application/json").body(include_str!("../openapi/openapi.json"))
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| format!("sqlite://{}/continuum.db", env!("CARGO_MANIFEST_DIR")));

    let pool = db::connect(&database_url)
        .await
        .expect("connexion à la base de données SQLite impossible");
    db::seed_if_empty(&pool)
        .await
        .expect("échec de l'initialisation des données de démonstration");

    println!("CONTINUUM API — http://127.0.0.1:8000");
    println!("Base de données — {database_url}");
    println!("Spécification OpenAPI — http://127.0.0.1:8000/api-docs/openapi.json");
    println!("(collez cette URL sur https://editor.swagger.io pour l'explorer visuellement)");

    let state = web::Data::new(AppState { pool });

    HttpServer::new(move || {
        // Autorise le front React (dev server Vite, port 5173) à appeler l'API.
        let cors = Cors::default()
            .allowed_origin("http://localhost:5173")
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(state.clone())
            .wrap(cors)
            .service(health)
            .service(openapi_spec)
            .service(list_branches)
            .service(get_branch)
            .service(diff_branches)
    })
    .bind(("127.0.0.1", 8000))?
    .run()
    .await
}
