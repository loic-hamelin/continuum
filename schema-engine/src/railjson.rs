//! Modèle RailJSON minimal.
//!
//! On ne modélise que les champs dont on a besoin pour l'extraction
//! spatiale et le futur layout schématique. `serde` ignore silencieusement
//! tous les champs non déclarés d'un objet JSON (slopes, curves,
//! extensions détaillées, etc.) : pas besoin de tout retranscrire pour
//! pouvoir lire un fichier RailJSON réel.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Une géométrie GeoJSON de type LineString : une suite de points
/// [longitude, latitude] qui dessine le tracé réel d'un tronçon de voie.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LineString {
    #[serde(rename = "type")]
    pub geom_type: String,
    pub coordinates: Vec<[f64; 2]>,
}

/// Un tronçon de voie : le brique de base du graphe topologique.
/// `length` est la longueur réelle en mètres (peut différer légèrement de
/// la longueur euclidienne de `geo`, qui est en degrés lon/lat).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrackSection {
    pub id: String,
    pub length: f64,
    pub geo: LineString,
}

/// Une branche (port) d'un appareil de voie : à quel tronçon elle se
/// connecte, et à quelle extrémité de ce tronçon (BEGIN ou END).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SwitchPort {
    pub endpoint: String,
    pub track: String,
}

/// Un appareil de voie (aiguillage). Relie plusieurs tronçons entre eux
/// par leurs extrémités — jamais en plein milieu d'un tronçon.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Switch {
    pub id: String,
    pub switch_type: String,
    pub ports: HashMap<String, SwitchPort>,
}

/// Un heurtoir : extrémité de voie "en cul-de-sac".
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BufferStop {
    pub id: String,
    pub track: String,
    pub position: f64,
}

/// Un détecteur (utilisé aussi comme point de repère de signalisation).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Detector {
    pub id: String,
    pub track: String,
    pub position: f64,
}

/// L'infrastructure RailJSON complète, réduite aux objets qui nous
/// intéressent pour l'extraction et le futur schéma.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RailInfra {
    pub track_sections: Vec<TrackSection>,
    #[serde(default)]
    pub switches: Vec<Switch>,
    #[serde(default)]
    pub buffer_stops: Vec<BufferStop>,
    #[serde(default)]
    pub detectors: Vec<Detector>,
}

impl RailInfra {
    /// Charge un fichier RailJSON depuis le disque. Le fichier peut être
    /// volumineux (le réseau belge complet fait ~30 Mo) : on lit en
    /// flux plutôt que de tout charger en `String` d'abord.
    pub fn load_from_file(path: &str) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let infra: RailInfra = serde_json::from_reader(reader)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(infra)
    }
}
