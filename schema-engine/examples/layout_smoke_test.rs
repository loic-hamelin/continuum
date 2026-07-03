//! Test manuel du layout schématique, à la suite de l'extraction (voir
//! smoke_test.rs). Usage : cargo run --example layout_smoke_test -- <chemin_railjson> <min_lon> <min_lat> <max_lon> <max_lat>

use continuum_schema_engine::{compute_layout, extract_bbox, Bbox, RailInfra};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let min_lon: f64 = args[2].parse().unwrap();
    let min_lat: f64 = args[3].parse().unwrap();
    let max_lon: f64 = args[4].parse().unwrap();
    let max_lat: f64 = args[5].parse().unwrap();

    let infra = RailInfra::load_from_file(path).expect("échec du chargement");
    let bbox = Bbox { min_lon, min_lat, max_lon, max_lat };
    let schema = extract_bbox(&infra, bbox);
    println!("Extraction : {} tronçons, {} aiguillages, {} heurtoirs", schema.tracks.len(), schema.switches.len(), schema.buffer_stops.len());

    let layout = compute_layout(&schema);
    println!("\n--- Layout schématique ---");
    println!("Nœuds : {}", layout.nodes.len());
    println!("Arêtes : {}", layout.edges.len());

    let max_lane = layout.edges.iter().map(|e| e.lane).max().unwrap_or(0);
    println!("Nombre de voies schématiques (lanes) : {}", max_lane + 1);

    let min_x = layout.nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
    let max_x = layout.nodes.iter().map(|n| n.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = layout.nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
    let max_y = layout.nodes.iter().map(|n| n.y).fold(f64::NEG_INFINITY, f64::max);
    println!("Étendue x : {min_x:.1} .. {max_x:.1}");
    println!("Étendue y : {min_y:.1} .. {max_y:.1}");

    let kinds: std::collections::HashMap<&str, usize> = layout.nodes.iter().fold(
        std::collections::HashMap::new(),
        |mut acc, n| { *acc.entry(n.kind.as_str()).or_insert(0) += 1; acc }
    );
    println!("Types de nœuds : {kinds:?}");

    if let Some(sample) = layout.nodes.first() {
        println!("\nExemple de nœud : {} ({}) -> x={:.1} y={:.1}", sample.id, sample.kind, sample.x, sample.y);
    }
}
