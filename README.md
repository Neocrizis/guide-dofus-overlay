# Guide Dofus Overlay

Appli Windows qui affiche le **journal du [Guide Dofus](https://neocrizis.github.io/guide-dofus/)** par-dessus le jeu,
comme le journal de quêtes : fenêtre sans cadre, fond transparent réglable, toujours au premier plan.

**[Télécharger la dernière version](https://github.com/Neocrizis/guide-dofus-overlay/releases/latest)** (fichier `…_x64-setup.exe`).

## Utilisation

- Déplacer : par la barre « Journal de quêtes ». Redimensionner : par les bords. La taille et la position sont retenues.
- ◐ : transparence du fond.
- **Ctrl+Maj+G** : afficher / masquer le journal.
- **Ctrl+Maj+T** : clic au travers (la souris passe à travers pour jouer dessous) ; recommencer pour revenir.
- Icône près de l'horloge : les mêmes réglages, « Toujours au premier plan », « Recharger », « Quitter ».
- **Cocher les quêtes terminées** (menu de l'icône, coupé par défaut) : l'appli lit la zone du chat que tu choisis (« Choisir la zone du chat… »)
  et coche dans le guide les quêtes terminées et succès déverrouillés. Lecture locale (OCR de Windows), aucune action sur le jeu.
  Le jeu doit être en mode fenêtré ou fenêtré plein écran, et le chat visible.
- 👤 : connexion à ton compte Guide Dofus pour synchroniser ta progression avec le site (facultatif).

Pour désinstaller : Paramètres Windows > Applications > Guide Dofus Overlay.

## Code signing policy

See [CODE_SIGNING_POLICY.md](CODE_SIGNING_POLICY.md). Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

## Confidentialité

Voir [PRIVACY.md](PRIVACY.md).

## Développement

Appli [Tauri 2](https://tauri.app) (Rust). Le contenu affiché est la page web du Guide Dofus (`?mode=overlay`) ;
ce dépôt ne contient que la fenêtre. Chaque push sur `main` qui change la version (`src-tauri/tauri.conf.json`)
construit l'installateur avec GitHub Actions et le publie dans les Releases.

```
npm install
npx tauri dev
```

Licence : [MIT](LICENSE). Dofus est une marque d'Ankama ; ce projet n'est ni affilié ni approuvé par Ankama.
