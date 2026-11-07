# 0016. The add-on owns its .blend libraries, each with a text image and a test

- Status: accepted
- Date: 2026-11-07
- Scope: Blender add-on (`resources/*.blend`, `runtime/importer/groups.py`)
- Supersedes in part: [0015](0015-addon-grafts-upstream-parts-keeps-item-path.md) (the POM chain is no longer left
  open: the fork keeps its own)

## Context
The add-on loads its node groups from binary libraries (`material_templates.blend`, `pom_library.blend`). The fork's
`pom_library.blend` is still the file of the fork point; fleetyards and this fork reshape its POM group at load time in
Python (`groups.py`: "Layers" removed, "UV Scale X/Y" and a detail control added), which depends on the file keeping
the exact names and wiring. Upstream maintains its own file instead and fixed POM inside it (d2b93f0e Bias
compensation and REPEAT sampler, a6c9d44d inversion on UV-mirrored faces). A binary file cannot be reviewed or merged
by git: a conflict is "all theirs or all ours", and a change is invisible in a pull request.

## Decision
- The fork maintains its own `.blend` libraries rather than reshaping groups at load time. `pom_library.blend` gets the
  fork's interface (UV Scale X/Y, detail control) and upstream's POM fixes built into its groups; the load-time POM
  rewrite in `groups.py` goes.
- Next to every library lies a text image: a JSON dump of its node groups (nodes, types, settings, default values,
  links, nested groups), written by a headless-Blender script. A test fails when a library and its image disagree, so
  every change to a `.blend` shows as a readable diff, and comparing with upstream's file is a text diff.
- Libraries are edited in Blender by hand or by a headless script; the image is regenerated with the change.
- `material_templates.blend` follows the same pattern from its next change on.

## Alternatives
- Keep reshaping at load time: fragile against any change of the file, and upstream's in-file fixes cannot be taken.
- Take upstream's file as it is: the fork's UV-scale interface and detail control would have to be rebuilt anyway.
- Build the groups entirely in Python: text and mergeable, but node networks are hard to read and author in code.

## Consequences
- Taking upstream changes to a library is a comparison of the two text images, then the same change in the fork's file.
- The test needs Blender (headless); it runs where Blender is available, the pure-Python suite stays as it is.
