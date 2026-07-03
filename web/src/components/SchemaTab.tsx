import { useEffect, useRef, useState } from "react";
import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { along, lineString } from "@turf/turf";
import type { Feature, FeatureCollection } from "geojson";
import {
  continuumApi,
  type Bbox,
  type ExtractedSchema,
  type ExtractedTrackSegment,
  type SchematicLayout,
} from "../api";

// Style vectoriel gratuit, sans clé API (mêmes tuiles que MapPair.tsx,
// pour rester cohérent visuellement entre les deux onglets).
const MAP_STYLE_URL = "https://tiles.openfreemap.org/styles/liberty";

// Anvers-Central — grande gare terminus, riche en heurtoirs et
// embranchements, utile comme zone de test pour l'extraction.
const ANTWERP_CENTER: [number, number] = [4.4214, 51.2162];

// Taille minimale d'un rectangle (en degrés) pour le considérer comme une
// vraie sélection plutôt qu'un simple clic accidentel.
const MIN_SELECTION_SPAN = 0.0005;

const EMPTY_FC: FeatureCollection = { type: "FeatureCollection", features: [] };

function findSegment(schema: ExtractedSchema, trackId: string): ExtractedTrackSegment | undefined {
  return schema.tracks.find((t) => t.track_id === trackId);
}

function endpointCoord(segment: ExtractedTrackSegment, endpoint: string): [number, number] | null {
  if (endpoint === "BEGIN" && segment.is_track_start) return segment.coordinates[0];
  if (endpoint === "END" && segment.is_track_end)
    return segment.coordinates[segment.coordinates.length - 1];
  return null;
}

/** Position interpolée le long d'un morceau de voie, via turf (même
 * technique que `MapPair.tsx` pour les objets ponctuels sur une voie). */
function interpolatedCoord(segment: ExtractedTrackSegment, position: number): [number, number] {
  if (segment.coordinates.length < 2) return segment.coordinates[0];
  const span = segment.end_position - segment.start_position;
  const clamped = Math.min(segment.end_position, Math.max(segment.start_position, position));
  const distanceFromStart = span > 0 ? clamped - segment.start_position : 0;
  const point = along(lineString(segment.coordinates), distanceFromStart, { units: "meters" });
  return point.geometry.coordinates as [number, number];
}

/** Convertit le résultat d'extraction en couches GeoJSON pour MapLibre :
 * tracés (avec propriété `cut`), et points pour aiguillages / heurtoirs /
 * détecteurs / marqueurs de coupure. */
function schemaToGeoJson(schema: ExtractedSchema) {
  const tracks: FeatureCollection = {
    type: "FeatureCollection",
    features: schema.tracks.map((t) => ({
      type: "Feature",
      properties: { cut: !t.is_track_start || !t.is_track_end },
      geometry: { type: "LineString", coordinates: t.coordinates },
    })),
  };

  const cutPoints: Feature[] = [];
  for (const t of schema.tracks) {
    if (!t.is_track_start) {
      cutPoints.push({ type: "Feature", properties: {}, geometry: { type: "Point", coordinates: t.coordinates[0] } });
    }
    if (!t.is_track_end) {
      cutPoints.push({
        type: "Feature",
        properties: {},
        geometry: { type: "Point", coordinates: t.coordinates[t.coordinates.length - 1] },
      });
    }
  }

  const switches: Feature[] = [];
  for (const sw of schema.switches) {
    const firstPort = Object.values(sw.ports)[0];
    if (!firstPort) continue;
    const segment = findSegment(schema, firstPort.track);
    if (!segment) continue;
    const coord = endpointCoord(segment, firstPort.endpoint);
    if (!coord) continue;
    switches.push({ type: "Feature", properties: { id: sw.id }, geometry: { type: "Point", coordinates: coord } });
  }

  const bufferStops: Feature[] = [];
  for (const bs of schema.buffer_stops) {
    const segment = findSegment(schema, bs.track);
    if (!segment) continue;
    const nearStart = Math.abs(bs.position - segment.start_position) <= Math.abs(bs.position - segment.end_position);
    const coord = (nearStart ? endpointCoord(segment, "BEGIN") : endpointCoord(segment, "END")) ?? interpolatedCoord(segment, bs.position);
    bufferStops.push({ type: "Feature", properties: { id: bs.id }, geometry: { type: "Point", coordinates: coord } });
  }

  const detectors: Feature[] = [];
  for (const d of schema.detectors) {
    const segment = findSegment(schema, d.track);
    if (!segment) continue;
    const coord = interpolatedCoord(segment, d.position);
    detectors.push({ type: "Feature", properties: { id: d.id }, geometry: { type: "Point", coordinates: coord } });
  }

  return {
    tracks,
    cutPoints: { type: "FeatureCollection", features: cutPoints } as FeatureCollection,
    switches: { type: "FeatureCollection", features: switches } as FeatureCollection,
    bufferStops: { type: "FeatureCollection", features: bufferStops } as FeatureCollection,
    detectors: { type: "FeatureCollection", features: detectors } as FeatureCollection,
  };
}

function rectangleGeoJson(c1: [number, number], c2: [number, number]): FeatureCollection {
  const west = Math.min(c1[0], c2[0]);
  const east = Math.max(c1[0], c2[0]);
  const south = Math.min(c1[1], c2[1]);
  const north = Math.max(c1[1], c2[1]);
  return {
    type: "FeatureCollection",
    features: [
      {
        type: "Feature",
        properties: {},
        geometry: {
          type: "Polygon",
          coordinates: [[[west, south], [east, south], [east, north], [west, north], [west, south]]],
        },
      },
    ],
  };
}

/**
 * Carte MapLibre (même style vectoriel qu'ailleurs dans l'app) avec
 * sélection rectangulaire par cliquer-glisser.
 */
function MapView({
  selecting,
  onSelectionComplete,
  extracted,
}: {
  selecting: boolean;
  onSelectionComplete: (bbox: Bbox) => void;
  extracted: ExtractedSchema | null;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);
  const draggingRef = useRef(false);
  const corner1Ref = useRef<[number, number] | null>(null);
  const corner2Ref = useRef<[number, number] | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    if (!containerRef.current) return;
    const map = new maplibregl.Map({
      container: containerRef.current,
      style: MAP_STYLE_URL,
      center: ANTWERP_CENTER,
      zoom: 15,
      attributionControl: false,
    });
    map.addControl(new maplibregl.NavigationControl(), "top-right");
    mapRef.current = map;

    map.on("load", () => {
      map.addSource("selection-rect", { type: "geojson", data: EMPTY_FC });
      map.addLayer({ id: "selection-fill", type: "fill", source: "selection-rect", paint: { "fill-color": "#2b6cb0", "fill-opacity": 0.08 } });
      map.addLayer({ id: "selection-outline", type: "line", source: "selection-rect", paint: { "line-color": "#2b6cb0", "line-width": 1 } });

      map.addSource("extracted-tracks", { type: "geojson", data: EMPTY_FC });
      map.addLayer({
        id: "extracted-tracks-layer",
        type: "line",
        source: "extracted-tracks",
        paint: { "line-color": ["case", ["get", "cut"], "#e8833a", "#2f7d4f"], "line-width": 4 },
      });

      map.addSource("extracted-cuts", { type: "geojson", data: EMPTY_FC });
      map.addLayer({ id: "extracted-cuts-layer", type: "circle", source: "extracted-cuts", paint: { "circle-radius": 4, "circle-color": "#fff", "circle-stroke-color": "#e8833a", "circle-stroke-width": 2 } });

      map.addSource("extracted-switches", { type: "geojson", data: EMPTY_FC });
      map.addLayer({ id: "extracted-switches-layer", type: "circle", source: "extracted-switches", paint: { "circle-radius": 5, "circle-color": "#2b6cb0" } });

      map.addSource("extracted-buffer-stops", { type: "geojson", data: EMPTY_FC });
      map.addLayer({ id: "extracted-buffer-stops-layer", type: "circle", source: "extracted-buffer-stops", paint: { "circle-radius": 5, "circle-color": "#c53030" } });

      map.addSource("extracted-detectors", { type: "geojson", data: EMPTY_FC });
      map.addLayer({ id: "extracted-detectors-layer", type: "circle", source: "extracted-detectors", paint: { "circle-radius": 3, "circle-color": "#718096" } });

      setReady(true);
    });

    return () => {
      map.remove();
      mapRef.current = null;
    };
  }, []);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready) return;
    const canvas = map.getCanvas();

    const handleMouseDown = (e: maplibregl.MapMouseEvent) => {
      if (!selecting) return;
      draggingRef.current = true;
      const c: [number, number] = [e.lngLat.lng, e.lngLat.lat];
      corner1Ref.current = c;
      corner2Ref.current = c;
      map.dragPan.disable();
    };
    const handleMouseMove = (e: maplibregl.MapMouseEvent) => {
      if (!selecting || !draggingRef.current) return;
      corner2Ref.current = [e.lngLat.lng, e.lngLat.lat];
      const src = map.getSource("selection-rect") as maplibregl.GeoJSONSource | undefined;
      if (src && corner1Ref.current && corner2Ref.current) {
        src.setData(rectangleGeoJson(corner1Ref.current, corner2Ref.current));
      }
    };
    const handleMouseUp = () => {
      if (!selecting || !draggingRef.current) return;
      draggingRef.current = false;
      map.dragPan.enable();
      const c1 = corner1Ref.current;
      const c2 = corner2Ref.current;
      if (!c1 || !c2) return;
      const bbox: Bbox = {
        min_lon: Math.min(c1[0], c2[0]),
        max_lon: Math.max(c1[0], c2[0]),
        min_lat: Math.min(c1[1], c2[1]),
        max_lat: Math.max(c1[1], c2[1]),
      };
      if (bbox.max_lon - bbox.min_lon < MIN_SELECTION_SPAN || bbox.max_lat - bbox.min_lat < MIN_SELECTION_SPAN) return;
      onSelectionComplete(bbox);
    };

    if (selecting) {
      canvas.style.cursor = "crosshair";
    } else {
      canvas.style.cursor = "";
      (map.getSource("selection-rect") as maplibregl.GeoJSONSource | undefined)?.setData(EMPTY_FC);
    }

    map.on("mousedown", handleMouseDown);
    map.on("mousemove", handleMouseMove);
    map.on("mouseup", handleMouseUp);
    return () => {
      map.off("mousedown", handleMouseDown);
      map.off("mousemove", handleMouseMove);
      map.off("mouseup", handleMouseUp);
    };
  }, [selecting, ready, onSelectionComplete]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready) return;
    const layers = schemaToGeoJson(extracted ?? { tracks: [], switches: [], buffer_stops: [], detectors: [] });
    (map.getSource("extracted-tracks") as maplibregl.GeoJSONSource | undefined)?.setData(layers.tracks);
    (map.getSource("extracted-cuts") as maplibregl.GeoJSONSource | undefined)?.setData(layers.cutPoints);
    (map.getSource("extracted-switches") as maplibregl.GeoJSONSource | undefined)?.setData(layers.switches);
    (map.getSource("extracted-buffer-stops") as maplibregl.GeoJSONSource | undefined)?.setData(layers.bufferStops);
    (map.getSource("extracted-detectors") as maplibregl.GeoJSONSource | undefined)?.setData(layers.detectors);
  }, [extracted, ready]);

  return <div ref={containerRef} className="schema-map-canvas" />;
}

const KIND_COLOR: Record<string, string> = {
  switch: "#2b6cb0",
  buffer_stop: "#c53030",
  cut: "#e8833a",
};

/** Rendu SVG du layout schématique — coordonnées x/y arbitraires, plus
 * de fond géographique. */
function SchematicSvg({ layout }: { layout: SchematicLayout }) {
  const nodeById = new Map(layout.nodes.map((n) => [n.id, n]));
  const margin = 24;
  const maxX = Math.max(1, ...layout.nodes.map((n) => n.x));
  const maxY = Math.max(1, ...layout.nodes.map((n) => n.y));
  const width = maxX + margin * 2;
  const height = maxY + margin * 2;

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="schematic-svg" role="img" aria-label="Schéma d'infrastructure">
      <g transform={`translate(${margin}, ${margin})`}>
        {layout.edges.map((e, i) => {
          const from = nodeById.get(e.from);
          const to = nodeById.get(e.to);
          if (!from || !to) return null;
          return <line key={`${e.track_id}-${i}`} x1={from.x} y1={from.y} x2={to.x} y2={to.y} stroke="#2f7d4f" strokeWidth={3} strokeLinecap="round" />;
        })}
        {layout.nodes.map((n) => (
          <circle key={n.id} cx={n.x} cy={n.y} r={n.kind === "cut" ? 3 : 5} fill={KIND_COLOR[n.kind] ?? "#718096"} />
        ))}
      </g>
    </svg>
  );
}

export function SchemaTab() {
  const [selecting, setSelecting] = useState(false);
  const [extracted, setExtracted] = useState<ExtractedSchema | null>(null);
  const [layout, setLayout] = useState<SchematicLayout | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSelectionComplete = async (bbox: Bbox) => {
    setSelecting(false);
    setLoading(true);
    setError(null);
    try {
      const [extractedResult, layoutResult] = await Promise.all([
        continuumApi.extractSchema(bbox),
        continuumApi.layoutSchema(bbox),
      ]);
      setExtracted(extractedResult);
      setLayout(layoutResult);
    } catch (e) {
      setError(String(e));
      setExtracted(null);
      setLayout(null);
    } finally {
      setLoading(false);
    }
  };

  const cutEnds = extracted
    ? extracted.tracks.reduce((acc, t) => acc + (t.is_track_start ? 0 : 1) + (t.is_track_end ? 0 : 1), 0)
    : 0;
  const laneCount = layout ? Math.max(0, ...layout.edges.map((e) => e.lane)) + 1 : 0;

  return (
    <section className="schema-tab">
      <div className="schema-toolbar">
        <button
          type="button"
          className={`tab-button schema-select-button ${selecting ? "tab-button--active" : ""}`}
          onClick={() => setSelecting((v) => !v)}
        >
          {selecting ? "Cliquer-glisser sur la carte…" : "Sélectionner une zone"}
        </button>
        {loading && <span className="board-subtitle">Extraction et mise en forme en cours…</span>}
      </div>

      {error && <div className="banner banner--warning">{error}</div>}

      <div className="board">
        <MapView selecting={selecting} onSelectionComplete={handleSelectionComplete} extracted={extracted} />
      </div>

      {extracted && (
        <div className="board">
          <div className="board-header">
            <h2>Sélection extraite (contrôle géographique)</h2>
            <span className="board-subtitle">
              {extracted.tracks.length} tronçon(s) · {cutEnds} extrémité(s) coupée(s)
            </span>
          </div>
          <ul className="node-list">
            <li><span className="kind-badge kind-appareil">Aiguillages</span>{extracted.switches.length} conservé(s)</li>
            <li><span className="kind-badge kind-voie">Heurtoirs</span>{extracted.buffer_stops.length} conservé(s)</li>
            <li><span className="kind-badge kind-signal">Détecteurs</span>{extracted.detectors.length} conservé(s)</li>
          </ul>
        </div>
      )}

      {layout && (
        <div className="board">
          <div className="board-header">
            <h2>Schéma d'infrastructure</h2>
            <span className="board-subtitle">{laneCount} voie(s) schématique(s)</span>
          </div>
          <div className="schematic-svg-wrapper">
            <SchematicSvg layout={layout} />
          </div>
        </div>
      )}
    </section>
  );
}
