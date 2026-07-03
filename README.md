# CONTINUUM

**Système de gestion de versions et de cycle de vie temporel du jumeau numérique ferroviaire, fondé sur OSRD.**

> Dépôt privé — développement en cours. Candidature déposée dans le cadre de l'Appel à Projets 2026 FIF / IRT Railenium.

## Idée en une phrase

Appliquer au système ferroviaire (voies, appareils de voie, sillons, horaires, projets d'investissement...) les principes qui ont transformé l'industrie du logiciel avec les systèmes de gestion de versions de type Git : un état de référence validé, des branches pour explorer des hypothèses concurrentes, des commits qui tracent chaque décision, un mécanisme de comparaison (diff), une revue avant fusion (merge), et une navigation dans l'historique (time machine).

Le moteur de simulation open-source **OSRD** (et son modèle de données **RailJSON**) sert de moteur de calcul : il évalue automatiquement les conséquences (capacité, robustesse, performance) de chaque branche du graphe.

## Statut du projet

- [x] Cadre théorique rédigé (graphe versionné hétérogène, typologie des conflits, fonctions commit/branche/fusion/time machine)
- [x] Dossier de candidature AAP 2026 FIF/Railenium
- [x] Squelette du projet : workspace Rust (graph-engine, osrd-bridge, api, cli) + interface React (web) connectée à l'API
- [ ] Vérifier que `cargo build` passe bien sur votre machine (voir note dans CLAUDE.md — non vérifié en environnement de génération)
- [x] Modélisation complète du graphe d'objets ferroviaires versionnés (branches, merge)
- [x] Prototype d'intégration avec OSRD / RailJSON
- [x] Mécanisme de détection de conflits
- [x] Interface de navigation temporelle (time machine)
- [x] Persistance réelle derrière l'API (base de données plutôt que données en mémoire)

## Structure du dépôt

```
continuum/
├── docs/            théorie, architecture, notes de conception
├── graph-engine/    cœur en Rust : modèle de graphe versionné (nœuds, arêtes, commits, diff)
├── osrd-bridge/     crate Rust : intégration avec OSRD / RailJSON (moteur de simulation)
├── api/             service HTTP en Rust (actix-web), sur le modèle d'editoast dans OSRD —
│                    expose le graphe via une API REST + spécification OpenAPI
├── cli/             interface en ligne de commande, en Rust
├── web/             interface graphique, en React + TypeScript (Vite), qui consomme l'API
└── examples/        jeux de données d'exemple pour les démonstrateurs
```

## Pile technique

- **Moteur de graphe et logique métier : Rust** (workspace Cargo à la racine, 4 crates : `graph-engine`, `osrd-bridge`, `api`, `cli`).
- **API : Rust / actix-web** (`api/`), sur le modèle d'**editoast** dans OSRD — un service qui récupère les données du graphe et les expose via des endpoints HTTP (`/branches`, `/branches/{name}`, `/diff`), avec une spécification OpenAPI dans `api/openapi/openapi.json`.
- **Interface graphique : React + TypeScript** (Vite), dans `web/` — consomme l'API pour explorer le graphe, comparer des branches, et à terme naviguer dans l'historique (« time machine »). Un système de design minimal (`web/src/theme.css`) pose les bases visuelles, inspirées de l'esthétique des outils ferroviaires modernes type OSRD — à affiner une fois que vous aurez comparé avec l'interface réelle.
- **Interopérabilité** : format RailJSON (OSRD) en entrée/sortie, côté `osrd-bridge`.

## Démarrer en local

**1. L'API (Rust)** — dans un premier terminal :
```
cargo run -p continuum-api
```
Elle démarre sur `http://127.0.0.1:8000`. Spécification OpenAPI consultable sur `http://127.0.0.1:8000/api-docs/openapi.json` (à coller dans https://editor.swagger.io pour l'explorer visuellement).

Les données sont stockées dans un fichier SQLite (`api/continuum.db`, ignoré par Git). Rien à installer ni à lancer à part : au premier démarrage, l'API crée le fichier, applique automatiquement le schéma (migrations `sqlx`) et recrée les deux branches de démonstration. Les démarrages suivants réutilisent le même fichier — l'historique survit aux redémarrages.

Pour changer l'emplacement du fichier, définissez `DATABASE_URL` dans un fichier `api/.env` (ignoré par Git), par ex. `DATABASE_URL=sqlite:continuum.db`.

`sqlx-cli` n'est **pas nécessaire pour lancer l'API** (les migrations s'appliquent toutes seules au démarrage). Il devient utile uniquement si vous voulez écrire une *nouvelle* migration plus tard :
```
cargo install sqlx-cli --no-default-features --features rustls,sqlite
cd api
sqlx migrate add nom_de_la_migration
```

**2. L'interface (React)** — dans un second terminal :
```
cd web
npm install
npm run dev
```
Ouvrez `http://localhost:5173`. L'interface se connecte automatiquement à l'API.

En cas de conflit lors d'une fusion, l'action "Ouvrir dans l'éditeur OSRD" du panneau de résolution ouvre l'URL définie par la variable d'environnement Vite `VITE_OSRD_EDITOR_URL` (fichier `web/.env`), avec `http://localhost:4000` comme valeur par défaut si absente.

**3. Le CLI (facultatif)**
```
cargo run -p continuum-cli
```

## Licence

Non définie — dépôt privé pour l'instant. À décider avant toute ouverture publique du code (MIT, Apache 2.0, etc. sont les choix classiques pour un projet open-source appuyé sur un écosystème existant comme OSRD).
