# Codex review

This repository includes `AGENTS.md` with project-specific **Code Review Rules** covering measurement integrity, local-data boundaries and interactive reliability. Deterministic formatting, lint and test checks run in GitHub Actions.

## Repository settings

Automatic Codex review is not enabled for this repository yet. When enabling it, the suggested settings are:

| Setting | Value |
| --- | --- |
| Automatic code review | Enabled · All PRs |
| Review trigger | On every push |
| Automatic security review | Enabled · All PRs, whenever code review runs |
| Threat model | `docs/threat-model.md` |
| Security reporting threshold | High for automatic reviews; Medium for manual reviews |

These settings are stored in the connected Codex account, while the rules and threat model are versioned here.

## GitHub integration

Automatic reviews are an account / repository setting in Codex, not a switch in a committed TOML file:

1. Connect the repository to Codex, granting the GitHub integration access to this repository.
2. Open [Codex code review settings](https://chatgpt.com/settings/code-review), select this repository, and enable automatic review for all pull requests.
3. Set the review trigger to **On every push** so follow-up changes receive fresh review.

A maintainer can request a one-off review with a PR comment:

```text
@codex review
```

The repository instructions guide findings; CI remains responsible for mechanical checks. Do not add a required Codex status check until the integration actually reports one. No `OPENAI_API_KEY` GitHub secret is required for the native Codex GitHub integration.

For a local check, use the installed Codex CLI:

```bash
codex review --uncommitted
codex review --base main
```

See [OpenAI's GitHub review documentation](https://developers.openai.com/codex/integrations/github/) for supported settings and current behavior. Enabling review consumes the reviewing account's applicable Codex allowance; this project does not bundle an API-powered review workflow.
