# Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

## Team roles

This project is maintained by a single developer, who holds all roles.

| Role | Members |
| --- | --- |
| Committers and reviewers | [Neocrizis](https://github.com/Neocrizis) ([repository members](https://github.com/Neocrizis/guide-dofus-overlay/graphs/contributors)) |
| Approvers | [Neocrizis](https://github.com/Neocrizis) (repository owner) |

- Every release is built from this repository's source by GitHub Actions (`.github/workflows/build.yml`); no binary is built or uploaded by hand.
- Every signing request is approved manually by an approver.
- Contributions from people other than the committers are reviewed before being merged.
- All team members use multi-factor authentication on GitHub and SignPath.

## Privacy policy

See [PRIVACY.md](PRIVACY.md).

## System changes

The installer (NSIS, per-user, no administrator rights) installs the program in the user's profile, creates a Start menu shortcut,
registers the `guidedofus://` link scheme (used to come back to the app after signing in to an account in the web browser)
and adds an uninstaller listed in Windows' "Apps & features". Uninstalling removes all of these.
