//! Sous-ensemble du format RailJSON (schéma OSRD, version 3.5.3).
//!
//! Les noms de champs et leur casse (`snake_case`) reprennent exactement
//! ceux du vrai schéma JSON d'OSRD :
//! <https://raw.githubusercontent.com/OpenRailAssociation/osrd/dev/front/src/reducers/osrdconf/infra_schema.json>
//! — vérifiés aussi contre un vrai jeu de données ("small_infra", utilisé
//! comme fixture de test dans `osrd-bridge/tests/`).
//!
//! RailJSON décrit uniquement l'infrastructure physique. Seuls
//! `TrackSection`, `Switch` et `Signal` sont modélisés finement ici — ce
//! sont les seuls types d'objets qui correspondent aux `NodeKind`
//! CONTINUUM `Voie`, `AppareilDeVoie` et `Signal` (voir `lib.rs`). Les 8
//! autres catégories obligatoires du document RailJSON (`routes`,
//! `buffer_stops`, `detectors`, `operational_points`, `speed_sections`,
//! `electrifications`, `level_crossings`, `neutral_sections`) n'ont pas
//! d'équivalent CONTINUUM aujourd'hui ; elles sont conservées comme des
//! tableaux JSON opaques uniquement pour produire un document conforme au
//! schéma (toujours vides en sortie de `export_to_railjson`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Version du schéma RailJSON couverte par ce module.
pub const RAILJSON_VERSION: &str = "3.5.3";

fn default_version() -> String {
    RAILJSON_VERSION.to_string()
}

/// Extrémité d'une voie (`TrackEndpoint.endpoint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Endpoint {
    #[serde(rename = "BEGIN")]
    Begin,
    #[serde(rename = "END")]
    End,
}

/// Sens d'utilisation d'un signal (`Signal.direction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    #[serde(rename = "START_TO_STOP")]
    StartToStop,
    #[serde(rename = "STOP_TO_START")]
    StopToStart,
}

/// Géométrie GeoJSON `LineString` (coordonnées `[lon, lat]` ou
/// `[lon, lat, alt]`) — le champ `geo`, obligatoire sur `TrackSection`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineString {
    #[serde(rename = "type")]
    pub geometry_type: String,
    pub coordinates: Vec<Vec<f64>>,
}

impl LineString {
    pub fn new(coordinates: Vec<Vec<f64>>) -> Self {
        Self {
            geometry_type: "LineString".to_string(),
            coordinates,
        }
    }
}

/// Rayon de courbure sur une portion de voie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    pub begin: f64,
    pub end: f64,
    pub radius: f64,
}

/// Pente/rampe sur une portion de voie (en mètres par kilomètre).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slope {
    pub begin: f64,
    pub end: f64,
    pub gradient: f64,
}

/// Limite de gabarit de chargement sur une portion de voie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingGaugeLimit {
    pub begin: f64,
    pub end: f64,
    /// Catégorie de gabarit (ex: "GB1", "G1", "FR3.3") — simplifié en
    /// chaîne plutôt qu'un enum fermé, pour ne pas devoir lister toutes
    /// les catégories réelles utilisées par OSRD.
    pub category: String,
}

/// Une portion de voie — correspond au `NodeKind::Voie` de CONTINUUM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackSection {
    pub id: String,
    pub length: f64,
    pub geo: LineString,
    #[serde(default)]
    pub slopes: Vec<Slope>,
    #[serde(default)]
    pub curves: Vec<Curve>,
    #[serde(default)]
    pub loading_gauge_limits: Vec<LoadingGaugeLimit>,
}

/// Référence à l'extrémité d'une voie (`Switch.ports`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackEndpoint {
    pub track: String,
    pub endpoint: Endpoint,
}

/// Un aiguillage — correspond au `NodeKind::AppareilDeVoie` de CONTINUUM.
///
/// `ports` est une map nommée (ex: "A", "B1", "B2" dans le vrai jeu de
/// données OSRD) plutôt qu'une simple liste : le nom du port identifie un
/// branchement précis de l'aiguillage, indépendamment de son type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Switch {
    pub id: String,
    pub switch_type: String,
    pub group_change_delay: f64,
    pub ports: HashMap<String, TrackEndpoint>,
}

/// Un signal logique bundlé dans un signal physique. Le vrai schéma
/// distingue 5 systèmes de signalisation (BAL, BAPR, TVM300, TVM430,
/// ETCS niveau 2) via une union discriminée sur `signaling_system` ; leurs
/// champs se recoupant tous (settings/paramètres en clé-valeur), on les
/// modélise ici avec une seule structure générique plutôt que 5 variantes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogicalSignal {
    pub signaling_system: String,
    #[serde(default)]
    pub next_signaling_systems: Vec<String>,
    #[serde(default)]
    pub settings: HashMap<String, String>,
    #[serde(default)]
    pub default_parameters: HashMap<String, String>,
    #[serde(default)]
    pub conditional_parameters: Vec<serde_json::Value>,
}

/// Un signal physique — correspond au `NodeKind::Signal` de CONTINUUM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub track: String,
    pub position: f64,
    pub direction: Direction,
    pub sight_distance: f64,
    #[serde(default)]
    pub logical_signals: Vec<LogicalSignal>,
}

/// Le document RailJSON complet (infrastructure). Seuls `track_sections`,
/// `switches` et `signals` sont peuplés par CONTINUUM ; les autres
/// catégories restent des tableaux JSON opaques (voir le commentaire
/// d'en-tête du module).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RailJsonInfra {
    #[serde(default = "default_version")]
    pub version: String,

    pub track_sections: Vec<TrackSection>,
    pub switches: Vec<Switch>,
    pub signals: Vec<Signal>,

    #[serde(default)]
    pub extended_switch_types: Vec<serde_json::Value>,

    // Catégories RailJSON hors périmètre de cette étape : pas de NodeKind
    // CONTINUUM correspondant aujourd'hui. Toujours vides en sortie
    // d'export ; leur contenu éventuel en entrée d'import est ignoré.
    #[serde(default)]
    pub buffer_stops: Vec<serde_json::Value>,
    #[serde(default)]
    pub detectors: Vec<serde_json::Value>,
    #[serde(default)]
    pub electrifications: Vec<serde_json::Value>,
    #[serde(default)]
    pub level_crossings: Vec<serde_json::Value>,
    #[serde(default)]
    pub neutral_sections: Vec<serde_json::Value>,
    #[serde(default)]
    pub operational_points: Vec<serde_json::Value>,
    #[serde(default)]
    pub routes: Vec<serde_json::Value>,
    #[serde(default)]
    pub speed_sections: Vec<serde_json::Value>,
}

impl Default for RailJsonInfra {
    fn default() -> Self {
        Self {
            version: default_version(),
            track_sections: Vec::new(),
            switches: Vec::new(),
            signals: Vec::new(),
            extended_switch_types: Vec::new(),
            buffer_stops: Vec::new(),
            detectors: Vec::new(),
            electrifications: Vec::new(),
            level_crossings: Vec::new(),
            neutral_sections: Vec::new(),
            operational_points: Vec::new(),
            routes: Vec::new(),
            speed_sections: Vec::new(),
        }
    }
}
