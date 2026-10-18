# Publishing npm packages

> This describes the upstream project's release process. This fork has no npm package or prebuilt release yet, and the `token-speed` name on npm belongs to upstream: publish under a different package name, or not at all.

Cargo users can install a tagged release directly from Git. The npm package ships the four native binaries from that same GitHub release, plus a small Node launcher. It has no runtime dependencies or install scripts; installation does not fetch binaries from GitHub. Linux needs glibc 2.35 or newer; Windows and musl are not supported.

The native TUI handles SIGINT, SIGTERM and SIGHUP by exiting its event loop normally, allowing its terminal guard to restore raw mode, mouse capture and the alternate screen. The npm launcher forwards these signals to the child. Native signal tests use Python's standard-library PTY support and empty synthetic log directories.

After publishing a verified GitHub release, run from the repository root with Node 18+, npm, GitHub CLI and tar available:

```sh
# Keep npm/package.json and Cargo.toml versions equal.
gh release download v0.3.1 --repo william1010121/token-speed \
  --pattern '*.tar.gz' --pattern SHA256SUMS --dir dist/npm-release
node scripts/prepare-npm.mjs dist/npm-release
npm pack ./npm --pack-destination dist
```

The preparation script checks each archive against the release's SHA256SUMS before extracting only the binary. The prepack check refuses a package missing any supported binary. Generated binaries, copied documentation and tarballs are ignored by Git.

Smoke-test the actual tarball without reading usage logs:

```sh
test_prefix=$(mktemp -d)
npm install --global --prefix "$test_prefix" --ignore-scripts ./dist/token-speed-0.3.1.tgz
"$test_prefix/bin/token-speed" --version
"$test_prefix/bin/token-speed" --help
```

Inspect `npm publish ./dist/token-speed-0.3.1.tgz --dry-run`, then authenticate using `npm login --auth-type=web`. Complete any security key / two-factor prompt in the browser. Publish the tested tarball:

```sh
npm publish ./dist/token-speed-0.3.1.tgz --access public
npm view token-speed version
```

Update the version in these commands for each release. Publishing an npm version is permanent; a released version cannot be overwritten. No registry credentials belong in the repository.
