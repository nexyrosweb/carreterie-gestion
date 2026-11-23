# Installation — Carreterie Gestion 1.0.0

Logiciel Windows hors ligne pour le Théâtre de la Carreterie.  
Éditeur : **Nexyros**.

## Prérequis

- Windows 10 (64 bits) ou Windows 11
- [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (souvent déjà présent avec Edge)

Aucun VPS, serveur web, abonnement ou service cloud n’est requis.

## Installation

1. Exécuter l’installateur `Carreterie Gestion_1.0.0_x64-setup.exe` (NSIS).
2. Suivre l’assistant (installation utilisateur recommandée).
3. Lancer **Carreterie Gestion** depuis le menu Démarrer.

## Première ouverture

1. Créer le compte administrateur (identifiant + mot de passe ≥ 12 caractères, généré et modifiable).
2. Compléter les **Paramètres** (coordonnées du théâtre, SIRET, régime de TVA, mentions légales).
3. Valider le régime fiscal de l’association avant de configurer les taux de TVA.

## Emplacement des données

Les données ne sont **jamais** stockées uniquement dans le dossier d’installation.

| Élément | Emplacement |
|--------|-------------|
| Base SQLite | `%APPDATA%\fr.nexyros.carreterie-gestion\carreterie.db` |
| Pièces jointes / PDF | `%APPDATA%\fr.nexyros.carreterie-gestion\attachments\` |
| Sauvegardes | `%APPDATA%\fr.nexyros.carreterie-gestion\backups\` |

## Désinstallation

La désinstallation retire le programme. Les données utilisateur dans `%APPDATA%` sont **conservées** volontairement.

## Sécurité

- Mot de passe applicatif haché avec **Argon2id** (jamais en clair).
- Verrouillage après 5 échecs (15 minutes) et après inactivité.
- Le mot de passe de l’application **ne remplace pas** le chiffrement du disque Windows (BitLocker recommandé).

## Avertissement réglementaire

Ce logiciel **n’est pas** une plateforme de facturation électronique agréée.  
Les factures internes gérées ici sont distinctes de l’émission / réception réglementaires via une plateforme agréée.
