//! Test manuel de vérification — pas un test automatisé (le fichier
//! RailJSON complet, ~30 Mo, n'est pas commité dans le dépôt).
//! Usage : cargo run --example smoke_test -- <chemin_railjson> <min_lon> <min_lat> <max_lon> <max_lat>

use continuum_schema_engine::{extract_bbox, Bbox, RailInfra};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let min_lon: f64 = args[2].parse().unwrap();
    let min_lat: f64 = args[3].parse().unwrap();
    let max_lon: f64 = args[4].parse().unwrap();
    let max_lat: f64 = args[5].parse().unwrap();

    println!("Chargement de {path}...");
    let infra = RailInfra::load_from_file(path).expect("échec du chargement");
    println!(
        "Infra chargée : {} tronçons, {} aiguillages, {} heurtoirs, {} détecteurs",
        infra.track_sections.len(),
        infra.switches.len(),
        infra.buffer_stops.len(),
        infra.detectors.len()
    );

    let bbox = Bbox { min_lon, min_lat, max_lon, max_lat };
    let schema = extract_bbox(&infra, bbox);

    println!("\n--- Extraction sur la sélection ---");
    println!("Morceaux de voie extraits : {}", schema.tracks.len());
    let cut_ends = schema
        .tracks
        .iter()
        .flat_map(|t| [t.is_track_start, t.is_track_end])
        .filter(|is_real_end| !is_real_end)
        .count();
    println!("Extrémités coupées par la sélection : {cut_ends}");
    println!("Aiguillages conservés (structure complète) : {}", schema.switches.len());
    println!("Heurtoirs conservés : {}", schema.buffer_stops.len());
    println!("Détecteurs conservés : {}", schema.detectors.len());

    if let Some(first) = schema.tracks.first() {
        println!("\nExemple de morceau : voie {} de {:.1}m à {:.1}m ({} points), extrémités réelles: début={} fin={}",
            first.track_id, first.start_position, first.end_position, first.coordinates.len(),
            first.is_track_start, first.is_track_end);
    }
}
