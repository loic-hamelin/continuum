//! Layout schématique : transforme le sous-graphe topologique extrait
//! (coordonnées géographiques réelles) en une représentation schématique
//! orthogonale simplifiée (coordonnées x/y arbitraires, lisibles).
//!
//! Méthode utilisée pour ce premier jet, inspirée du "linear schematic
//! drawing" (voir la bibliographie discutée avant de démarrer ce
//! projet) : on suppose que la zone sélectionnée s'organise
//! principalement autour d'un axe dominant (un corridor, une gare). Le
//! layout se fait en 3 étapes :
//!
//! 1. Trouver l'axe dominant du nuage de points (analyse en composantes
//!    principales, calcul direct puisqu'on est en 2D — pas besoin d'une
//!    librairie d'algèbre linéaire pour ça).
//! 2. Projeter chaque nœud (aiguillage, heurtoir, coupure) sur cet axe :
//!    ça donne la coordonnée x du schéma (position "le long du
//!    corridor").
//! 3. Affecter à chaque tronçon une "voie" (lane) au sens schématique,
//!    par coloration d'intervalles (algorithme glouton classique) pour
//!    que les tronçons parallèles ne se chevauchent pas verticalement.
//!    C'est aussi ce qui donne la coordonnée y.
//!
//! Limite connue : pour une zone avec plusieurs corridors qui se
//! croisent franchement (pas juste une gare avec des voies parallèles),
//! un seul axe dominant ne suffit plus — il faudrait alors un layout
//! octolinéaire façon carte de métro. Étape ultérieure si besoin.

use crate::extract::{ExtractedSchema, ExtractedTrackSegment};
use serde::Serialize;
use std::collections::HashMap;

const SCHEMATIC_WIDTH: f64 = 1000.0;
const LANE_SPACING: f64 = 24.0;

#[derive(Debug, Clone, Serialize)]
pub struct LayoutNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    /// "switch" | "buffer_stop" | "cut"
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayoutEdge {
    pub from: String,
    pub to: String,
    pub track_id: String,
    pub lane: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchematicLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
}

/// Cherche, parmi les aiguillages, celui dont un port touche `track_id`
/// à l'extrémité `endpoint` ("BEGIN"/"END"). Renvoie son identifiant.
fn switch_at(schema: &ExtractedSchema, track_id: &str, endpoint: &str) -> Option<String> {
    schema
        .switches
        .iter()
        .find(|sw| {
            sw.ports
                .values()
                .any(|p| p.track == track_id && p.endpoint == endpoint)
        })
        .map(|sw| sw.id.clone())
}

/// Cherche, parmi les heurtoirs, celui situé sur `track_id` à la
/// position `position` (à epsilon près).
fn buffer_stop_at(schema: &ExtractedSchema, track_id: &str, position: f64) -> Option<String> {
    schema
        .buffer_stops
        .iter()
        .find(|bs| bs.track == track_id && (bs.position - position).abs() < 1e-2)
        .map(|bs| bs.id.clone())
}

/// Détermine l'identifiant de nœud et la coordonnée géographique de
/// l'extrémité "début" d'un morceau de voie.
fn start_node(schema: &ExtractedSchema, seg: &ExtractedTrackSegment) -> (String, String, [f64; 2]) {
    let coord = seg.coordinates[0];
    if seg.is_track_start {
        if let Some(id) = switch_at(schema, &seg.track_id, "BEGIN") {
            return (format!("switch:{id}"), "switch".into(), coord);
        }
        if let Some(id) = buffer_stop_at(schema, &seg.track_id, seg.start_position) {
            return (format!("bufferstop:{id}"), "buffer_stop".into(), coord);
        }
    }
    (
        format!("cut:{}:{:.1}", seg.track_id, seg.start_position),
        "cut".into(),
        coord,
    )
}

/// Idem pour l'extrémité "fin".
fn end_node(schema: &ExtractedSchema, seg: &ExtractedTrackSegment) -> (String, String, [f64; 2]) {
    let coord = seg.coordinates[seg.coordinates.len() - 1];
    if seg.is_track_end {
        if let Some(id) = switch_at(schema, &seg.track_id, "END") {
            return (format!("switch:{id}"), "switch".into(), coord);
        }
        if let Some(id) = buffer_stop_at(schema, &seg.track_id, seg.end_position) {
            return (format!("bufferstop:{id}"), "buffer_stop".into(), coord);
        }
    }
    (
        format!("cut:{}:{:.1}", seg.track_id, seg.end_position),
        "cut".into(),
        coord,
    )
}

/// Axe dominant (vecteur unitaire) du nuage de points, par analyse en
/// composantes principales — calcul direct en 2D (matrice de covariance
/// 2x2, valeurs propres par la formule fermée du second degré).
fn principal_axis(coords: &[[f64; 2]]) -> [f64; 2] {
    let n = coords.len() as f64;
    let mean_x = coords.iter().map(|c| c[0]).sum::<f64>() / n;
    let mean_y = coords.iter().map(|c| c[1]).sum::<f64>() / n;
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for c in coords {
        let dx = c[0] - mean_x;
        let dy = c[1] - mean_y;
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let trace = sxx + syy;
    let det = sxx * syy - sxy * sxy;
    let disc = (trace * trace / 4.0 - det).max(0.0).sqrt();
    let lambda1 = trace / 2.0 + disc;
    let (vx, vy) = if sxy.abs() > 1e-12 {
        (sxy, lambda1 - sxx)
    } else if sxx >= syy {
        (1.0, 0.0)
    } else {
        (0.0, 1.0)
    };
    let norm = (vx * vx + vy * vy).sqrt();
    if norm < 1e-9 {
        [1.0, 0.0]
    } else {
        [vx / norm, vy / norm]
    }
}

/// Calcule le layout schématique d'un sous-graphe déjà extrait.
pub fn compute_layout(schema: &ExtractedSchema) -> SchematicLayout {
    // 1. Construction des nœuds + arêtes topologiques à partir des
    // morceaux de voie extraits.
    struct RawEdge {
        from: String,
        to: String,
        track_id: String,
    }
    let mut node_coords: HashMap<String, [f64; 2]> = HashMap::new();
    let mut node_kinds: HashMap<String, String> = HashMap::new();
    let mut raw_edges: Vec<RawEdge> = Vec::new();

    for seg in &schema.tracks {
        let (from_id, from_kind, from_coord) = start_node(schema, seg);
        let (to_id, to_kind, to_coord) = end_node(schema, seg);
        node_coords.entry(from_id.clone()).or_insert(from_coord);
        node_coords.entry(to_id.clone()).or_insert(to_coord);
        node_kinds.entry(from_id.clone()).or_insert(from_kind);
        node_kinds.entry(to_id.clone()).or_insert(to_kind);
        raw_edges.push(RawEdge { from: from_id, to: to_id, track_id: seg.track_id.clone() });
    }

    if node_coords.is_empty() {
        return SchematicLayout { nodes: vec![], edges: vec![] };
    }

    // 2. Projection sur l'axe dominant.
    let coords: Vec<[f64; 2]> = node_coords.values().copied().collect();
    let axis = principal_axis(&coords);
    let mean_x = coords.iter().map(|c| c[0]).sum::<f64>() / coords.len() as f64;
    let mean_y = coords.iter().map(|c| c[1]).sum::<f64>() / coords.len() as f64;

    let mut projected: HashMap<String, f64> = HashMap::new();
    for (id, coord) in &node_coords {
        let dx = coord[0] - mean_x;
        let dy = coord[1] - mean_y;
        projected.insert(id.clone(), dx * axis[0] + dy * axis[1]);
    }
    let min_proj = projected.values().cloned().fold(f64::INFINITY, f64::min);
    let max_proj = projected.values().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (max_proj - min_proj).max(1e-6);
    // x normalisé dans [0, SCHEMATIC_WIDTH]
    let x_of = |id: &str| -> f64 { (projected[id] - min_proj) / span * SCHEMATIC_WIDTH };

    // 3. Affectation de voie par coloration d'intervalles (glouton) :
    // chaque arête occupe l'intervalle [x_from, x_to] ; on lui donne la
    // première voie libre (celle dont la dernière arête se termine
    // avant le début de la nouvelle), sinon on ouvre une nouvelle voie.
    let mut edge_intervals: Vec<(usize, f64, f64)> = raw_edges
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let a = x_of(&e.from);
            let b = x_of(&e.to);
            (i, a.min(b), a.max(b))
        })
        .collect();
    edge_intervals.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let mut lane_ends: Vec<f64> = Vec::new(); // fin (x) de la dernière arête de chaque voie
    let mut edge_lane: Vec<usize> = vec![0; raw_edges.len()];
    for (idx, start, end) in edge_intervals {
        let mut chosen = None;
        for (lane_idx, lane_end) in lane_ends.iter().enumerate() {
            if *lane_end <= start + 1e-6 {
                chosen = Some(lane_idx);
                break;
            }
        }
        let lane_idx = match chosen {
            Some(l) => {
                lane_ends[l] = end;
                l
            }
            None => {
                lane_ends.push(end);
                lane_ends.len() - 1
            }
        };
        edge_lane[idx] = lane_idx;
    }

    // Coordonnée y d'un nœud = moyenne des voies des arêtes qui le
    // touchent (donne un raccord légèrement diagonal aux aiguillages,
    // ce qui est en fait la représentation habituelle d'un aiguillage
    // dans un schéma).
    let mut node_lane_sum: HashMap<String, (f64, usize)> = HashMap::new();
    for (i, e) in raw_edges.iter().enumerate() {
        let lane = edge_lane[i] as f64;
        let entry_from = node_lane_sum.entry(e.from.clone()).or_insert((0.0, 0));
        entry_from.0 += lane;
        entry_from.1 += 1;
        let entry_to = node_lane_sum.entry(e.to.clone()).or_insert((0.0, 0));
        entry_to.0 += lane;
        entry_to.1 += 1;
    }

    let nodes = node_coords
        .keys()
        .map(|id| {
            let (sum, count) = node_lane_sum.get(id).copied().unwrap_or((0.0, 1));
            let avg_lane = if count > 0 { sum / count as f64 } else { 0.0 };
            LayoutNode {
                id: id.clone(),
                x: x_of(id),
                y: avg_lane * LANE_SPACING,
                kind: node_kinds.get(id).cloned().unwrap_or_else(|| "cut".into()),
            }
        })
        .collect();

    let edges = raw_edges
        .into_iter()
        .enumerate()
        .map(|(i, e)| LayoutEdge { from: e.from, to: e.to, track_id: e.track_id, lane: edge_lane[i] })
        .collect();

    SchematicLayout { nodes, edges }
}
