import { useState } from "react";
import type { RailNode } from "../api";
import { KIND_ORDER, KindBadge } from "../nodeKind";

/** Une ligne à afficher : un nœud, avec une variante optionnelle pour le
 * contexte diff (ajout/suppression) — `undefined` pour un affichage neutre
 * (ex: l'état complet d'une branche). */
export interface NodeRow {
  node: RailNode;
  variant?: "added" | "removed";
}

interface NodeGroupListProps {
  rows: NodeRow[];
  emptyMessage: string;
  /** Hauteur maximale du board avant défilement interne, en pixels. */
  maxHeight?: number;
}

/**
 * Liste de nœuds groupée par type d'objet (`NodeKind`), en sections
 * repliables — évite qu'une branche avec ~150 objets (voies, aiguillages,
 * signaux réels d'une infra OSRD) rende la page interminable. Réutilisée à
 * la fois pour l'état complet d'une branche et pour le diff d'une fusion
 * (dans ce cas, chaque ligne garde son symbole +/− et sa couleur, mais le
 * regroupement par type reste commun aux deux usages).
 */
export function NodeGroupList({ rows, emptyMessage, maxHeight = 480 }: NodeGroupListProps) {
  const groups = new Map<string, NodeRow[]>();
  for (const row of rows) {
    const bucket = groups.get(row.node.kind);
    if (bucket) {
      bucket.push(row);
    } else {
      groups.set(row.node.kind, [row]);
    }
  }
  const nonEmptyKinds = KIND_ORDER.filter((kind) => (groups.get(kind)?.length ?? 0) > 0);

  // Replié par défaut, sauf le premier groupe non vide — calculé une seule
  // fois à l'affichage initial (pas recalculé si les données se
  // rafraîchissent ensuite, pour ne pas replier un groupe que
  // l'utilisateur vient d'ouvrir).
  const [expanded, setExpanded] = useState<Set<string>>(
    () => new Set(nonEmptyKinds.length > 0 ? [nonEmptyKinds[0]] : [])
  );

  const toggle = (kind: string) => {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (next.has(kind)) {
        next.delete(kind);
      } else {
        next.add(kind);
      }
      return next;
    });
  };

  if (rows.length === 0) {
    return (
      <ul className="node-list">
        <li className="node-empty">{emptyMessage}</li>
      </ul>
    );
  }

  return (
    <div className="node-group-list" style={{ maxHeight }}>
      {nonEmptyKinds.map((kind) => {
        const kindRows = groups.get(kind) ?? [];
        const isExpanded = expanded.has(kind);
        return (
          <div className="node-group" key={kind}>
            <button
              type="button"
              className="node-group-header"
              onClick={() => toggle(kind)}
              aria-expanded={isExpanded}
            >
              <span className="node-group-header-left">
                <KindBadge kind={kindRows[0].node.kind} />
                <span className="node-group-count">{kindRows.length}</span>
              </span>
              <span className="node-group-chevron">{isExpanded ? "▾" : "▸"}</span>
            </button>
            {isExpanded && (
              <ul className="node-list node-list-dense">
                {kindRows.map(({ node, variant }) => (
                  <li
                    key={node.id}
                    className={
                      variant === "added"
                        ? "node-added node-row-dense"
                        : variant === "removed"
                          ? "node-removed node-row-dense"
                          : "node-row-dense"
                    }
                  >
                    {variant === "added" && <span className="diff-symbol">+</span>}
                    {variant === "removed" && <span className="diff-symbol">−</span>}
                    {node.label}
                  </li>
                ))}
              </ul>
            )}
          </div>
        );
      })}
    </div>
  );
}
