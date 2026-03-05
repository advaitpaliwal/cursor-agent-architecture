# Cursor Agent Architecture

Public technical reference for Cursor's Background Agent runtime.

This repository is organized for GitHub readers first: start with curated docs in `docs/`, then use `extracted/` only if you want raw evidence files.

## Recommended Reading Order

1. [Key Findings](docs/KEY_FINDINGS.md)
2. [Full Architecture Reference](docs/ARCHITECTURE_REFERENCE.md)
3. [Documentation Index](docs/README.md)
4. [Session Wave Notes (raw appendix, optional)](docs/SESSION_WAVE_NOTES.md)
5. [Raw Evidence Index (optional)](extracted/README.md)

## What You Should Expect

- `docs/` contains human-readable architecture explanations.
- `extracted/` contains raw extracted files, logs, configs, and reverse-engineering artifacts.
- This is a research/archive repo, not Cursor's production source code.

## Repository Layout

| Path | Purpose |
| --- | --- |
| `docs/` | Curated docs for readers |
| `docs/KEY_FINDINGS.md` | Fast high-signal summary |
| `docs/ARCHITECTURE_REFERENCE.md` | Full deep-dive technical write-up |
| `extracted/` | Raw evidence snapshots and extracted artifacts |
| `extracted/ansible/` | Desktop provisioning playbook and assets |
| `extracted/exec-daemon-code/` | Protocol/tool/prompt extraction files |
| `extracted/system/` | Runtime package/env/tool inventories |
| `extracted/binary-analysis/` | Binary reverse-engineering notes |

## Cleanup Applied

- Replaced the monolithic root README with GitHub-friendly navigation.
- Moved the long-form reference into `docs/ARCHITECTURE_REFERENCE.md`.
- Added `docs/KEY_FINDINGS.md` for quick understanding.
- Added `docs/SESSION_WAVE_NOTES.md` to isolate dense raw inspection logs.
- Replaced key ASCII architecture/network diagrams with Mermaid for GitHub rendering.
- Removed the social-media-oriented "Twitter thread" section from the technical docs.
- Added explicit indexing so raw artifacts are optional, not required reading.
- Kept a single canonical Ansible README.

## Contributor Notes

- Put explanations in `docs/`.
- Keep `extracted/` as immutable evidence snapshots when possible.
- Add new evidence with descriptive filenames and update `extracted/README.md`.
