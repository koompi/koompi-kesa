# @marckrenn/pi-sub-core

## 1.0.4

### Patch Changes

- [#30](https://github.com/marckrenn/pi-sub/pull/30) [`af0828a`](https://github.com/marckrenn/pi-sub/commit/af0828a8d2e529497a1acff95e388a0a3eabb90e) Thanks [@marckrenn](https://github.com/marckrenn)! - Store the shared cache and lock files in the agent directory so all sub-core instances share a single cache.

- [#22](https://github.com/marckrenn/pi-sub/pull/22) [`3e5a026`](https://github.com/marckrenn/pi-sub/commit/3e5a026ea3dc113561ff32466a8aa03b91c6d876) Thanks [@marckrenn](https://github.com/marckrenn)! - Store sub-core and sub-bar settings in agent-level JSON files so updates no longer overwrite user configuration. Legacy extension `settings.json` files are migrated into the new files and removed after a successful migration.

  Manual migration (if you want to do it yourself before updating):

  ```
  cp ~/.pi/agent/extensions/sub-core/settings.json ~/.pi/agent/pi-sub-core-settings.json
  cp ~/.pi/agent/extensions/sub-bar/settings.json ~/.pi/agent/pi-sub-bar-settings.json
  ```

  Existing users should move legacy settings from the extension folders to:
  - `~/.pi/agent/pi-sub-core-settings.json`
  - `~/.pi/agent/pi-sub-bar-settings.json`

- [`a6c0d33`](https://github.com/marckrenn/pi-sub/commit/a6c0d33c8d19d2876a4a8a1a0a69302a3c63f5e8) Thanks [@marckrenn](https://github.com/marckrenn)! - Move the shared cache/lock files under `~/.pi/agent/cache/sub-core` so all clients share a single cache directory.

- [`7da1e08`](https://github.com/marckrenn/pi-sub/commit/7da1e082e634f4e4dee2560b4d490527d1543ade) Thanks [@marckrenn](https://github.com/marckrenn)! - Add a minimum refresh interval setting to cap refresh frequency even when refresh is triggered every turn.

- [`1f5e451`](https://github.com/marckrenn/pi-sub/commit/1f5e45173b9868b0d6645ae35a084142a0ac56a5) Thanks [@marckrenn](https://github.com/marckrenn)! - Gate tool registration behind `tools.usageTool` and `tools.allUsageTool` (default off) in sub-core settings.

- [`35eb185`](https://github.com/marckrenn/pi-sub/commit/35eb18590f369db4cda931b8e11099d0f3ddb4ec) Thanks [@marckrenn](https://github.com/marckrenn)! - Add usage tool aliases `get_current_usage` and `get_all_usage`.

- Updated dependencies [[`7da1e08`](https://github.com/marckrenn/pi-sub/commit/7da1e082e634f4e4dee2560b4d490527d1543ade)]:
  - @marckrenn/pi-sub-shared@1.0.4

## 1.0.3

### Patch Changes

- [`6fa2736`](https://github.com/marckrenn/pi-sub/commit/6fa27363573f34c38a372a6d7b8b74e756716724) Thanks [@marckrenn](https://github.com/marckrenn)! - Update extension tool execute signature order for compatibility with latest Pi API.

- Updated dependencies [[`6fa2736`](https://github.com/marckrenn/pi-sub/commit/6fa27363573f34c38a372a6d7b8b74e756716724)]:
  - @marckrenn/pi-sub-shared@1.0.3

## 1.0.2

### Patch Changes

- [#3](https://github.com/marckrenn/pi-sub/pull/3) [`4ceb5ad`](https://github.com/marckrenn/pi-sub/commit/4ceb5ad133166237652d197ba9296ad1589a813c) Thanks [@marckrenn](https://github.com/marckrenn)! - Bundle sub-core with sub-bar, refresh Antigravity quotas + settings, and update UI copy/controls.

- Updated dependencies [[`4ceb5ad`](https://github.com/marckrenn/pi-sub/commit/4ceb5ad133166237652d197ba9296ad1589a813c)]:
  - @marckrenn/pi-sub-shared@1.0.2

## 1.0.1

### Patch Changes

- Align repo version with npm publish.
- Updated dependencies:
  - @marckrenn/pi-sub-shared@1.0.1

## 1.0.0

### Major Changes

- [`9bedd80`](https://github.com/marckrenn/pi-sub/commit/9bedd80b0037b723e70f0376019fff59a617e7cb) Thanks [@marckrenn](https://github.com/marckrenn)! - Initial 1.0.0 release.

### Patch Changes

- Updated dependencies [[`9bedd80`](https://github.com/marckrenn/pi-sub/commit/9bedd80b0037b723e70f0376019fff59a617e7cb)]:
  - @marckrenn/pi-sub-shared@1.0.0
