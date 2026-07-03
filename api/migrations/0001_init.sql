-- Schéma initial de persistance CONTINUUM.
--
-- Chaque commit garde un instantané complet du graphe (comme le fait
-- graph-engine en mémoire, et comme Git le fait réellement avec ses objets
-- "tree") : commit_nodes/commit_edges ne sont pas des deltas.

-- Un commit : auteur, message, date.
CREATE TABLE commits (
    id         TEXT PRIMARY KEY,
    author     TEXT NOT NULL,
    message    TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Table d'adjacence pour les parents d'un commit : parent_order = 0 pour un
-- commit normal (un seul parent), 0 et 1 pour un commit de fusion (branche
-- cible puis branche source). C'est ainsi que Git modélise ses parents.
CREATE TABLE commit_parents (
    commit_id    TEXT NOT NULL REFERENCES commits(id),
    parent_id    TEXT NOT NULL REFERENCES commits(id),
    parent_order INTEGER NOT NULL,
    PRIMARY KEY (commit_id, parent_order)
);

-- Une branche : un nom qui pointe vers le commit courant.
CREATE TABLE branches (
    name      TEXT PRIMARY KEY,
    commit_id TEXT NOT NULL REFERENCES commits(id)
);

-- L'état des nœuds à un commit donné.
CREATE TABLE commit_nodes (
    commit_id  TEXT NOT NULL REFERENCES commits(id),
    node_id    TEXT NOT NULL,
    kind       TEXT NOT NULL,
    label      TEXT NOT NULL,
    properties TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (commit_id, node_id)
);

-- L'état des arêtes à un commit donné.
CREATE TABLE commit_edges (
    commit_id TEXT NOT NULL REFERENCES commits(id),
    from_node TEXT NOT NULL,
    to_node   TEXT NOT NULL,
    kind      TEXT NOT NULL,
    PRIMARY KEY (commit_id, from_node, to_node, kind)
);

CREATE INDEX idx_commit_parents_parent ON commit_parents(parent_id);
CREATE INDEX idx_branches_commit ON branches(commit_id);
