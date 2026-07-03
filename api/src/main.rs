//! CONTINUUM — api
//!
//! Service HTTP qui expose le graphe versionné via une API REST, sur le
//! modèle d'editoast dans OSRD (service Rust qui récupère les données et
//! les rend disponibles via des endpoints HTTP documentés).
//!
//! Les données sont persistées dans un fichier SQLite (voir `db.rs` et
//! `migrations/`) : l'historique des commits et des branches survit au
//! redémarrage du serveur. Les handlers HTTP vivent dans `routes.rs` ;
//! ce fichier ne fait que le câblage (connexion DB, démarrage du serveur).

mod db;
mod routes;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use sqlx::SqlitePool;

pub struct AppState {
    pub pool: SqlitePool,
    /// Infrastructure RailJSON chargée en mémoire au démarrage, utilisée
    /// uniquement par l'onglet "Schéma" (`continuum_schema_engine`) pour
    /// extraire un sous-graphe à partir d'une sélection géographique.
    /// Volontairement indépendante du graphe versionné SQLite ci-dessus :
    /// c'est un jeu de données brut (tout un pays), pas les objets suivis
    /// par CONTINUUM.
    pub schema_infra: continuum_schema_engine::RailInfra,
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

    let railjson_path = std::env::var("RAILJSON_PATH")
        .unwrap_or_else(|_| format!("{}/../data/belgium-260419_railjson.json", env!("CARGO_MANIFEST_DIR")));
    println!("Chargement de l'infrastructure RailJSON (onglet Schéma) — {railjson_path}");
    let schema_infra = continuum_schema_engine::RailInfra::load_from_file(&railjson_path).unwrap_or_else(|e| {
        panic!(
            "échec du chargement du fichier RailJSON à '{railjson_path}' ({e}) — \
             vérifie la variable RAILJSON_PATH dans ton fichier .env (voir .env.example)"
        )
    });
    println!("Infrastructure chargée — {} tronçons de voie", schema_infra.track_sections.len());

    println!("CONTINUUM API — http://127.0.0.1:8000");
    println!("Base de données — {database_url}");
    println!("Spécification OpenAPI — http://127.0.0.1:8000/api-docs/openapi.json");
    println!("(collez cette URL sur https://editor.swagger.io pour l'explorer visuellement)");

    let state = web::Data::new(AppState { pool, schema_infra });

    HttpServer::new(move || {
        // Autorise le front React (dev server Vite, port 5173) à appeler l'API.
        let cors = Cors::default()
            .allowed_origin("http://localhost:5173")
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(state.clone())
            .wrap(cors)
            .configure(routes::configure)
    })
    .bind(("127.0.0.1", 8000))?
    .run()
    .await
}
