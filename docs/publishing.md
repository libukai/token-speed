# Publishing npm packages

The npm package is `@libukai/token-speed`. (The unscoped `token-speed` on npm is the [upstream](https://github.com/william1010121/token-speed) project.) It ships the four native binaries from a GitHub release of this repository, plus a small Node launcher. It has no runtime dependencies or install scripts; installation does not fetch binaries from GitHub. Linux needs glibc 2.35 or newer; Windows and musl are not supported.

The native TUI handles SIGINT, SIGTERM and SIGHUP by exiting its event loop normally, allowing its terminal guard to restore raw mode, mouse capture and the alternate screen. The npm launcher forwards these signals to the child. Native signal tests use Python's standard-library PTY support and empty synthetic log directories.

After the release workflow has published a verified GitHub release, run from the repository root with Node 18+, npm, GitHub CLI and tar available:

```sh
# Keep npm/package.json and Cargo.toml versions equal.
version=0.5.0
gh release download "v$version" --repo libukai/token-speed \
  --pattern '*.tar.gz' --pattern SHA256SUMS --dir dist/npm-release
node scripts/prepare-npm.mjs dist/npm-release
npm pack ./npm --pack-destination dist
```

The preparation script checks each archive against the release's SHA256SUMS before extracting only the binary. The prepack check refuses a package missing any supported binary. Generated binaries, copied documentation and tarballs are ignored by Git.

Smoke-test the actual tarball without reading usage logs:

```sh
test_prefix=$(mktemp -d)
npm install --global --prefix "$test_prefix" --ignore-scripts "./dist/libukai-token-speed-$version.tgz"
"$test_prefix/bin/token-speed" --version
"$test_prefix/bin/token-speed" --help
```

Inspect `npm publish "./dist/libukai-token-speed-$version.tgz" --dry-run`, then authenticate using `npm login --auth-type=web`. Complete any security key / two-factor prompt in the browser. Publish the tested tarball:

```sh
npm publish "./dist/libukai-token-speed-$version.tgz" --access public
npm view @libukai/token-speed version
```

Publishing an npm version is permanent; a released version cannot be overwritten. No registry credentials belong in the repository.
