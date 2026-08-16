# Codex Project Instructions

## Implementation planning and doubt clarification

- Before starting each new implementation slice, the main Luna Max thread must
  first build a complete requirement view: expected behavior, constraints,
  repository touchpoints, proposed approach, and the concrete questions that
  could affect implementation.
- Before editing, explicitly delegate that requirement packet to the project
  custom agent `doubt-clarifier` from
  `.codex/agents/doubt-clarifier.toml`, and wait for its response.
- Use the Sol High response to resolve nuances, tradeoffs, edge cases, and
  validation requirements. Keep Luna Max as the sole implementation and
  verification owner.
- The doubt clarifier is read-only and must not edit files, fixtures, roadmap
  entries, or git state, or claim execution results.

The repository's detailed codec, safety, parity, coverage, and verification
contracts are documented in `CLAUDE.md`; apply those same contracts to Codex
work in this repository.
