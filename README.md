# Carreterie Gestion

Micro-logiciel Windows hors ligne pour la gestion des factures du **Théâtre de la Carreterie**.  
Développé par **Nexyros** — version **1.0.0**.

> Ce logiciel n’est pas une plateforme de facturation électronique agréée.

## Stack

- Tauri 2 · React · TypeScript · Tailwind CSS
- Rust · SQLite (migrations) · Argon2id · PDF local · sauvegardes chiffrées

## Développement

Prérequis : Node.js 20+, Rust stable, Visual Studio Build Tools (C++), WebView2.

```bash
npm install
npm run tauri dev
```

Build installateur NSIS :

```bash
npm run tauri build
```

Tests :

```bash
npm test
cd src-tauri && cargo test --lib
```

## Documentation

- [Installation](docs/INSTALLATION.md)
- [Manuel utilisateur](docs/MANUEL_UTILISATEUR.md)
- [Sauvegarde et restauration](docs/SAUVEGARDE_RESTAURATION.md)

## Données

Stockées dans `%APPDATA%\fr.nexyros.carreterie-gestion\` (jamais uniquement dans le dossier d’installation).
