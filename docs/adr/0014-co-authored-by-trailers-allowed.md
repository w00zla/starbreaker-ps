# 0014. Co-authored-by trailers are allowed

- Status: accepted
- Date: 2026-11-07
- Scope: repository policy

## Context
[0011](0011-follow-agents-git-rules.md) adopted AGENTS.md's rule of no generated trailers in commits. In practice
the squash merges #1-#3 carry GitHub's own `Co-authored-by` trailer (the author address of the branch commits
differed from the account's), and the maintainer works with an AI assistant whose commits name it as co-author.

## Decision
- `Co-authored-by` trailers are allowed in the fork's commits and squash merges: GitHub's own ones and the AI
  assistant's.
- Everything else of 0011 stays: branch from `main`, pull request, squash merge.
- AGENTS.md stays as upstream ships it (no merge conflicts on syncs); this record is the fork's deviation from its
  trailer rule.

## Alternatives
- Keep 0011's rule and strip trailers: extra care on every squash for no gain in a single-maintainer fork.
- Edit AGENTS.md: a lasting conflict with upstream on every sync.

## Consequences
- The existing trailers on #1-#3 are within the rules; nothing is rewritten.
- Pull requests sent to upstream follow upstream's rules (no trailers).
