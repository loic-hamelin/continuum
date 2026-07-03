import { useEffect, useRef } from "react";
import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { along, lineString } from "@turf/turf";
import type { Feature, FeatureCollection, LineString, Point } from "geojson";
import type { Graph } from "../api";

// Style vectoriel gratuit, sans clé API (tuiles OSM) — voir https://openfreemap.org
const MAP_STYLE_URL = "https://tiles.openfreemap.org/styles/liberty";

const TRACK_SOURCE_ID = "tracks";
const TRACK_LAYER_ID = "tracks-line";
const POINTS_SOURCE_ID = "points";
const POINTS_LAYER_ID = "points-circle";

/** Une géométrie GeoJSON `LineString`, telle que stockée par `osrd-bridge`
 * dans `RailNode.properties.geo` (voir osrd-bridge/src/railjson.rs). */
interface GeoLineString {
  type: "LineString";
  coordinates: [number, number][];
}

function trackLines(graph: Graph): { features: Feature<LineString>[]; tracksById: Map<string, LineString> } {
  const features: Feature<LineString>[] = [];
  const tracksById = new Map<string, LineString>();
  for (const node of Object.values(graph.nodes)) {
    if (node.kind !== "Voie") continue;
    const geo = node.properties.geo as GeoLineString | undefined;
    if (!geo || geo.type !== "LineString") continue;
    const geometry: LineString = { type: "LineString", coordinates: geo.coordinates };
    tracksById.set(node.id, geometry);
    features.push({
      type: "Feature",
      properties: { id: node.id },
      geometry,
    });
  }
  return { features, tracksById };
}

/** Interpole la position d'un objet ponctuel (signal, aiguillage...) le
 * long de la géométrie de sa voie, à partir de `properties.track` +
 * `properties.position` (mètres) — la convention établie par `osrd-bridge`. */
function pointFeatures(
  graph: Graph,
  tracksById: Map<string, LineString>,
  conflictIds: Set<string>
): Feature<Point>[] {
  const features: Feature<Point>[] = [];
  for (const node of Object.values(graph.nodes)) {
    if (node.kind === "Voie") continue;
    const track = node.properties.track as string | undefined;
    const position = node.properties.position as number | undefined;
    if (track === undefined || position === undefined) continue;
    const trackLine = tracksById.get(track);
    if (!trackLine) continue;

    const point = along(lineString(trackLine.coordinates), position, { units: "meters" });
    features.push({
      type: "Feature",
      properties: {
        id: node.id,
        label: node.label,
        kind: node.kind,
        conflicting: conflictIds.has(node.id),
      },
      geometry: point.geometry,
    });
  }
  return features;
}

function featureCollection<G extends LineString | Point>(features: Feature<G>[]): FeatureCollection<G> {
  return { type: "FeatureCollection", features };
}

interface SingleMapProps {
  label: string;
  graph: Graph | null;
  conflictIds: string[];
  mapRef: React.MutableRefObject<maplibregl.Map | null>;
  onUserMove: (center: maplibregl.LngLat, zoom: number) => void;
}

function SingleMap({ label, graph, conflictIds, mapRef, onUserMove }: SingleMapProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const suppressMoveRef = useRef(false);

  useEffect(() => {
    if (!containerRef.current) return;

    const map = new maplibregl.Map({
      container: containerRef.current,
      style: MAP_STYLE_URL,
      center: [2.3, 48.85],
      zoom: 12,
      attributionControl: false,
    });
    map.addControl(new maplibregl.NavigationControl(), "top-right");
    mapRef.current = map;

    map.on("move", () => {
      if (suppressMoveRef.current) return;
      onUserMove(map.getCenter(), map.getZoom());
    });

    return () => {
      map.remove();
      mapRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Expose une méthode pour appliquer un centre/zoom sans redéclencher onUserMove.
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;
    (map as unknown as { __suppressMoveRef?: typeof suppressMoveRef }).__suppressMoveRef = suppressMoveRef;
  });

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !graph) return;

    const conflictSet = new Set(conflictIds);
    const { features: lines, tracksById } = trackLines(graph);
    const points = pointFeatures(graph, tracksById, conflictSet);

    const applyData = () => {
      const trackSource = map.getSource(TRACK_SOURCE_ID) as maplibregl.GeoJSONSource | undefined;
      const pointSource = map.getSource(POINTS_SOURCE_ID) as maplibregl.GeoJSONSource | undefined;

      if (trackSource) {
        trackSource.setData(featureCollection(lines));
      } else {
        map.addSource(TRACK_SOURCE_ID, { type: "geojson", data: featureCollection(lines) });
        map.addLayer({
          id: TRACK_LAYER_ID,
          type: "line",
          source: TRACK_SOURCE_ID,
          paint: { "line-color": "#1844EF", "line-width": 2 },
        });
      }

      if (pointSource) {
        pointSource.setData(featureCollection(points));
      } else {
        map.addSource(POINTS_SOURCE_ID, { type: "geojson", data: featureCollection(points) });
        map.addLayer({
          id: POINTS_LAYER_ID,
          type: "circle",
          source: POINTS_SOURCE_ID,
          paint: {
            "circle-radius": ["case", ["get", "conflicting"], 9, 5],
            "circle-color": ["case", ["get", "conflicting"], "#D91C1C", "#085953"],
            "circle-stroke-width": ["case", ["get", "conflicting"], 3, 1],
            "circle-stroke-color": ["case", ["get", "conflicting"], "#FF6868", "#ffffff"],
          },
        });
      }

      if (lines.length > 0) {
        const bounds = new maplibregl.LngLatBounds();
        for (const feature of lines) {
          for (const coord of feature.geometry.coordinates) {
            bounds.extend(coord as [number, number]);
          }
        }
        map.fitBounds(bounds, { padding: 40, animate: false });
      }
    };

    if (map.isStyleLoaded()) {
      applyData();
    } else {
      map.once("load", applyData);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [graph, conflictIds.join(",")]);

  return (
    <div className="map-pane">
      <div className="map-pane-label">{label}</div>
      <div ref={containerRef} className="map-canvas" />
    </div>
  );
}

interface MapPairProps {
  sourceLabel: string;
  targetLabel: string;
  sourceGraph: Graph | null;
  targetGraph: Graph | null;
  sourceConflictIds: string[];
  targetConflictIds: string[];
}

/** Deux cartes MapLibre séparées, zoom/position synchronisés — plus
 * lisible qu'une carte unique avec overlay dès qu'il y a plusieurs
 * conflits proches, et cohérent avec la présentation d'un merge Git en
 * deux colonnes. */
export function MapPair({
  sourceLabel,
  targetLabel,
  sourceGraph,
  targetGraph,
  sourceConflictIds,
  targetConflictIds,
}: MapPairProps) {
  const sourceMapRef = useRef<maplibregl.Map | null>(null);
  const targetMapRef = useRef<maplibregl.Map | null>(null);

  const syncTo = (target: React.MutableRefObject<maplibregl.Map | null>) => (center: maplibregl.LngLat, zoom: number) => {
    const map = target.current;
    if (!map) return;
    const suppress = (map as unknown as { __suppressMoveRef?: React.MutableRefObject<boolean> }).__suppressMoveRef;
    if (suppress) suppress.current = true;
    map.jumpTo({ center, zoom });
    if (suppress) suppress.current = false;
  };

  return (
    <div className="board">
      <div className="board-header">
        <h2>Comparaison spatiale</h2>
        <span className="board-subtitle">déplacement synchronisé</span>
      </div>
      <div className="map-pair">
        <SingleMap
          label={sourceLabel}
          graph={sourceGraph}
          conflictIds={sourceConflictIds}
          mapRef={sourceMapRef}
          onUserMove={syncTo(targetMapRef)}
        />
        <SingleMap
          label={targetLabel}
          graph={targetGraph}
          conflictIds={targetConflictIds}
          mapRef={targetMapRef}
          onUserMove={syncTo(sourceMapRef)}
        />
      </div>
    </div>
  );
}
