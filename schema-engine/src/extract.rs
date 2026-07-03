//! Extraction d'un sous-graphe topologique à partir d'une sélection
//! rectangulaire de l'utilisateur.
//!
//! Une voie sélectionnée en partie seulement est découpée : le morceau
//! conservé garde la trace de si chaque extrémité correspond à la vraie
//! fin de la voie (`is_track_start`/`is_track_end`) ou à une coupure
//! introduite par le rectangle de sélection — utile ensuite pour dessiner
//! une flèche "la voie continue hors cadre" dans le rendu schématique.

use crate::geometry::{clip_segment, lerp, lerp_point, positions_along_track, Bbox};
use crate::railjson::RailInfra;
use serde::Serialize;
use std::collections::HashMap;

const EPSILON: f64 = 1e-6;

/// Un morceau de voie conservé après découpage par la sélection.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractedTrackSegment {
    pub track_id: String,
    pub start_position: f64,
    pub end_position: f64,
    pub coordinates: Vec<[f64; 2]>,
    /// `true` si `start_position` correspond à la vraie extrémité de la
    /// voie (BEGIN) plutôt qu'à une coupure due à la sélection.
    pub is_track_start: bool,
    /// idem pour `end_position` / END.
    pub is_track_end: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractedSchema {
    pub tracks: Vec<ExtractedTrackSegment>,
    pub switches: Vec<crate::railjson::Switch>,
    pub buffer_stops: Vec<crate::railjson::BufferStop>,
    pub detectors: Vec<crate::railjson::Detector>,
}

/// Découpe une voie par la sélection et renvoie les morceaux conservés
/// (il peut y en avoir plusieurs si la voie ressort puis rentre dans la
/// zone de sélection).
fn clip_track(
    track: &crate::railjson::TrackSection,
    bbox: &Bbox,
) -> Vec<ExtractedTrackSegment> {
    let coords = &track.geo.coordinates;
    if coords.len() < 2 {
        return vec![];
    }
    let positions = positions_along_track(coords, track.length);

    let mut runs: Vec<ExtractedTrackSegment> = vec![];
    let mut current: Option<ExtractedTrackSegment> = None;

    for i in 0..coords.len() - 1 {
        let p0 = coords[i];
        let p1 = coords[i + 1];
        match clip_segment(p0, p1, bbox) {
            None => {
                if let Some(run) = current.take() {
                    runs.push(finalize_run(run, track.length));
                }
            }
            Some((t0, t1)) => {
                let entry_pos = lerp(positions[i], positions[i + 1], t0);
                let exit_pos = lerp(positions[i], positions[i + 1], t1);
                let entry_pt = lerp_point(p0, p1, t0);
                let exit_pt = lerp_point(p0, p1, t1);

                if current.is_none() {
                    current = Some(ExtractedTrackSegment {
                        track_id: track.id.clone(),
                        start_position: entry_pos,
                        end_position: entry_pos,
                        coordinates: vec![entry_pt],
                        is_track_start: false,
                        is_track_end: false,
                    });
                } else if t0 > EPSILON {
                    // Le rectangle est convexe : ce cas (rentrer à nouveau
                    // alors qu'un morceau est déjà ouvert) ne devrait pas
                    // arriver en pratique, mais on reste défensif.
                    if let Some(run) = current.take() {
                        runs.push(finalize_run(run, track.length));
                    }
                    current = Some(ExtractedTrackSegment {
                        track_id: track.id.clone(),
                        start_position: entry_pos,
                        end_position: entry_pos,
                        coordinates: vec![entry_pt],
                        is_track_start: false,
                        is_track_end: false,
                    });
                }

                let run = current.as_mut().unwrap();
                run.coordinates.push(exit_pt);
                run.end_position = exit_pos;

                if t1 < 1.0 - EPSILON {
                    // Le segment ressort du rectangle avant son extrémité :
                    // on ferme le morceau ici.
                    runs.push(finalize_run(current.take().unwrap(), track.length));
                }
            }
        }
    }
    if let Some(run) = current.take() {
        runs.push(finalize_run(run, track.length));
    }
    runs
}

fn finalize_run(mut run: ExtractedTrackSegment, real_length: f64) -> ExtractedTrackSegment {
    run.is_track_start = run.start_position.abs() < 1e-3;
    run.is_track_end = (real_length - run.end_position).abs() < 1e-3;
    run
}

/// Extrait le sous-graphe topologique contenu dans `bbox`.
pub fn extract_bbox(infra: &RailInfra, bbox: Bbox) -> ExtractedSchema {
    let mut tracks: Vec<ExtractedTrackSegment> = vec![];
    // track_id -> liste des morceaux conservés (utilisée pour savoir si un
    // aiguillage/heurtoir/détecteur tombe dans une zone conservée).
    let mut kept_runs: HashMap<&str, Vec<&ExtractedTrackSegment>> = HashMap::new();

    for track in &infra.track_sections {
        // Rejet rapide : si la boîte englobante du tracé complet ne
        // touche même pas la sélection, inutile de découper segment par
        // segment.
        let Some(track_bbox) = Bbox::of_coordinates(&track.geo.coordinates) else {
            continue;
        };
        if !track_bbox.intersects(&bbox) {
            continue;
        }
        let runs = clip_track(track, &bbox);
        for run in runs {
            tracks.push(run);
        }
    }
    for run in &tracks {
        kept_runs.entry(&run.track_id).or_default().push(run);
    }

    let track_end_present = |track_id: &str, endpoint: &str| -> bool {
        kept_runs.get(track_id).is_some_and(|runs| {
            runs.iter().any(|r| match endpoint {
                "BEGIN" => r.is_track_start,
                "END" => r.is_track_end,
                _ => false,
            })
        })
    };

    // Un aiguillage n'est conservé que si TOUTES ses branches sont
    // présentes dans la sélection — sinon la structure de la jonction
    // serait incomplète et trompeuse à afficher.
    let switches: Vec<_> = infra
        .switches
        .iter()
        .filter(|sw| {
            sw.ports
                .values()
                .all(|port| track_end_present(&port.track, &port.endpoint))
        })
        .cloned()
        .collect();

    let position_in_kept_run = |track_id: &str, position: f64| -> bool {
        kept_runs.get(track_id).is_some_and(|runs| {
            runs.iter()
                .any(|r| position >= r.start_position - 1e-3 && position <= r.end_position + 1e-3)
        })
    };

    let buffer_stops: Vec<_> = infra
        .buffer_stops
        .iter()
        .filter(|bs| position_in_kept_run(&bs.track, bs.position))
        .cloned()
        .collect();

    let detectors: Vec<_> = infra
        .detectors
        .iter()
        .filter(|d| position_in_kept_run(&d.track, d.position))
        .cloned()
        .collect();

    ExtractedSchema { tracks, switches, buffer_stops, detectors }
}
