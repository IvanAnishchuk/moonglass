# Reftest Assets

This directory contains a small verbatim subset of the pinned
`ethereum/consensus-specs` vector release used by the reftest runner.

The layout intentionally mirrors the upstream archive:

`consensus-specs-v1.7.0-alpha.11/tests/<preset>/<fork>/<runner>/<handler>/<suite>/<case>/`

Unit and integration tests use these files instead of constructing synthetic
`manifest.yaml`, `meta.yaml`, or `steps.yaml` fixtures.

Preset fixtures in this subset are Gloas fixtures only. Shared `general`
fixtures are included for runner families that upstream stores outside a
preset/fork tree, such as BLS and KZG.

`manifest.json` records the copied case directories and SHA256 for every file
under the pinned release root. Update it whenever the asset subset changes.
