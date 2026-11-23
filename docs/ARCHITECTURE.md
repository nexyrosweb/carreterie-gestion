# Architecture — Carreterie Gestion

## Vue d’ensemble

Application **Tauri 2** (shell Windows + WebView2) :

- **Frontend** : React + TypeScript strict + Tailwind (UI française, thème sombre Carreterie)
- **Backend** : Rust — SQLite, auth, métier, PDF, sauvegardes
- **Données** : `%APPDATA%\fr.nexyros.carreterie-gestion\` uniquement

## Modules Rust

| Module | Rôle |
|--------|------|
| `auth` | Argon2id, tentatives, sessions mémoire, inactivité |
| `db` | Pool SQLite + migrations v1/v2 |
| `services/invoices` | Brouillons, émission, avoirs, règlements, immutabilité |
| `services/suppliers` | Factures fournisseurs + pièces jointes |
| `services/pdf` | PDF facture/avoir (mentions FR) |
| `services/backup` | VACUUM, ZIP, AES-GCM, SHA-256 |
| `services/import` | Import PDF, hash, matching contacts, validation |
| `services/pdf_text` / `pdf_parse` | Extraction texte + parseur FR + OCR Tesseract optionnel |
| `services/audit` | Journal des opérations sensibles |
| `commands` | Surface `invoke` Tauri (session obligatoire) |

## Règles métier clés

- Facture **émise** : numéro unique, non éditable / non supprimable
- Corrections via **avoirs**
- Calculs monétaires en `rust_decimal` (EUR, 2 décimales)
- SQL paramétré uniquement
- Non plateforme agréée (disclaimer systématique)

## Frontend

Navigation latérale : Tableau de bord, Factures clients, Factures fournisseurs, Contacts, Archives, Sauvegardes, Paramètres (+ À propos).
