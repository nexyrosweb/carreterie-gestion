# Module Import PDF — Carreterie Gestion

Importation et analyse hors ligne des factures PDF (clients, acomptes, avoirs, fournisseurs).

## Fonctionnement

1. Menu **Import PDF** → choisir un ou plusieurs fichiers `.pdf` (ou un ZIP).
2. Analyse locale : extraction de texte (`pdf-extract`), OCR optionnel via **Tesseract** (`tesseract` + pack `fra` sur le PATH).
3. **PDF multi-documents** : si plusieurs factures / acomptes / avoirs sont dans le même fichier (une page = un document, ou en-têtes `Facture N°` / `Avoir N°`), l’import crée **une fiche par document**, avec client, type et montants détectés séparément.
4. Vérification manuelle obligatoire (aperçu PDF + formulaire) pour chaque fiche.
5. Validation → archivage dans SQLite + conservation du PDF original.
6. Aucune facture officielle numérotée n’est créée automatiquement.

## Règles

- Doublons : empreinte **SHA-256** du fichier + contrôle métier (n° + SIRET).
- Champs absents ou incertains : laissés vides / signalés, jamais inventés.
- Correspondance contact : SIREN/SIRET prioritaire, puis nom — **pas de fusion automatique**.
- Acomptes / avoirs : type détecté + référence d’origine si présente.
- Option « enregistrement lié » : brouillon client (non émis) ou facture fournisseur uniquement.

## Stockage

`%APPDATA%\fr.nexyros.carreterie-gestion\attachments\imports\{hash}.pdf`

Inclus dans les sauvegardes (dossier `attachments`).

## OCR (scans)

Sans couche texte, installez [Tesseract](https://github.com/UB-Mannheim/tesseract/wiki) avec la langue française, puis redémarrez l’application. Sinon, saisie manuelle après import.
