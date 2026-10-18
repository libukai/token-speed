# Security

The current 0.3.x line receives fixes. The application parses local agent logs and writes a local SQLite metadata cache; it does not contact external services.

Report security-sensitive bugs through [GitHub private vulnerability reporting](https://github.com/libukai/token-speed/security/advisories/new). For ordinary correctness bugs, use the issue tracker. Include a minimal synthetic reproduction, the affected version and the impact. Do not upload credentials, prompts, private logs or cache databases.

The index and exports contain session IDs and source paths. Redact this metadata when sharing reports. Release archives include `SHA256SUMS` for integrity verification; checksums alone are not independent publisher authentication.
