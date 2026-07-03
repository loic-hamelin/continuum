//! Fonctions géométriques nécessaires à l'extraction : boîte englobante
//! et découpage d'un segment de droite par un rectangle (l'algorithme de
//! Liang-Barsky, un classique du graphisme 2D).

/// Une zone de sélection rectangulaire, en coordonnées géographiques
/// (longitude/latitude, comme dans les fichiers RailJSON).
#[derive(Debug, Clone, Copy)]
pub struct Bbox {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

impl Bbox {
    pub fn of_coordinates(coords: &[[f64; 2]]) -> Option<Bbox> {
        let mut iter = coords.iter();
        let first = iter.next()?;
        let mut bbox = Bbox { min_lon: first[0], max_lon: first[0], min_lat: first[1], max_lat: first[1] };
        for c in iter {
            bbox.min_lon = bbox.min_lon.min(c[0]);
            bbox.max_lon = bbox.max_lon.max(c[0]);
            bbox.min_lat = bbox.min_lat.min(c[1]);
            bbox.max_lat = bbox.max_lat.max(c[1]);
        }
        Some(bbox)
    }

    pub fn intersects(&self, other: &Bbox) -> bool {
        self.min_lon <= other.max_lon
            && self.max_lon >= other.min_lon
            && self.min_lat <= other.max_lat
            && self.max_lat >= other.min_lat
    }
}

/// Découpe le segment [p0, p1] par le rectangle `bbox` (algorithme de
/// Liang-Barsky). Renvoie la portion du segment [t0, t1] (paramétrée de
/// 0.0 à 1.0 entre p0 et p1) qui se trouve à l'intérieur du rectangle,
/// ou `None` si le segment ne croise pas le rectangle du tout.
pub fn clip_segment(p0: [f64; 2], p1: [f64; 2], bbox: &Bbox) -> Option<(f64, f64)> {
    let dx = p1[0] - p0[0];
    let dy = p1[1] - p0[1];
    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;

    // Les 4 côtés du rectangle : (p, q) tel que le point est valide si
    // t*p <= q. Voir la présentation classique de Liang-Barsky.
    let checks = [
        (-dx, p0[0] - bbox.min_lon),
        (dx, bbox.max_lon - p0[0]),
        (-dy, p0[1] - bbox.min_lat),
        (dy, bbox.max_lat - p0[1]),
    ];

    for (p, q) in checks {
        if p == 0.0 {
            if q < 0.0 {
                return None; // segment parallèle à ce côté et hors du rectangle
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                if r > t0 {
                    t0 = r;
                }
            } else {
                if r < t0 {
                    return None;
                }
                if r < t1 {
                    t1 = r;
                }
            }
        }
    }

    if t0 > t1 {
        None
    } else {
        Some((t0, t1))
    }
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn lerp_point(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t)]
}

/// Convertit les sommets bruts d'un tracé (en degrés lon/lat) en positions
/// linéaires le long de la voie, en mètres, recalées pour que le dernier
/// sommet corresponde exactement à `real_length` (le champ `length` du
/// RailJSON). Approximation raisonnable pour un usage de visualisation :
/// on suppose que la distance parcourue est proportionnelle à la distance
/// euclidienne cumulée en coordonnées géographiques.
pub fn positions_along_track(coords: &[[f64; 2]], real_length: f64) -> Vec<f64> {
    let mut cumulative = vec![0.0_f64; coords.len()];
    for i in 1..coords.len() {
        let dx = coords[i][0] - coords[i - 1][0];
        let dy = coords[i][1] - coords[i - 1][1];
        cumulative[i] = cumulative[i - 1] + (dx * dx + dy * dy).sqrt();
    }
    let total = *cumulative.last().unwrap_or(&0.0);
    if total <= 0.0 {
        return cumulative; // tracé dégénéré (un seul point) — improbable mais on ne divise pas par zéro
    }
    cumulative.iter().map(|d| d / total * real_length).collect()
}
