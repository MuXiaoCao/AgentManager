# Codex CLI Support

Status: archived after local testing passed.

## Goal

AgentManager should support Codex CLI alongside Claude Code:

- live session cards through hooks
- local history scanning
- resume in iTerm
- hook setup/status UI
- existing Claude behavior preserved

## Decisions

- Install Codex hooks at user level in `~/.codex/hooks.json` so all trusted projects can be tracked.
- Preserve existing Codex hooks, including unrelated hook groups, and append AgentManager entries idempotently.
- Use the existing HTTP endpoint, session card model, aliases, iTerm jump, and iTerm arrange flow.
- Treat Codex `Stop` as idle, not ended. Codex does not expose a documented `SessionEnd` hook.
- Resume Claude with `claude --resume <session_id>` and Codex with `codex resume <session_id> --cd <cwd>`.

## Implementation

- Generalize hook installation/status into per-agent entries.
- Extend the shared hook script to parse Claude and Codex payloads defensively.
- Add a Codex history scanner for `~/.codex/sessions/YYYY/MM/DD/*.jsonl`.
- Generalize the frontend history list to switch between Claude and Codex.
- Update setup banner and i18n copy to reference both agents.

## Verification

- `cargo test --lib`
- `npx tsc --noEmit`
- `npm run build`
- `npm run tauri:build`
- Manual local test: install hooks, start `codex` in iTerm, verify card appears and history resume works.
