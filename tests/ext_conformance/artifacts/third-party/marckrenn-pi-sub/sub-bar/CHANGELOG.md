# @marckrenn/pi-sub-bar

## 1.0.4

### Patch Changes

- [#28](https://github.com/marckrenn/pi-sub/pull/28) [`2e35657`](https://github.com/marckrenn/pi-sub/commit/2e3565776a98ec133537fe0bacbc099cd4afadbe) Thanks [@marckrenn](https://github.com/marckrenn)! - Default Anthropic “Show Extra Window” provider setting to off.

- [#29](https://github.com/marckrenn/pi-sub/pull/29) [`7e6b7a0`](https://github.com/marckrenn/pi-sub/commit/7e6b7a08d69cf1cd456d2add406ec89c5c86f5df) Thanks [@marckrenn](https://github.com/marckrenn)! - Split display padding into left/right settings and migrate legacy paddingX values to both sides.

- [#22](https://github.com/marckrenn/pi-sub/pull/22) [`3e5a026`](https://github.com/marckrenn/pi-sub/commit/3e5a026ea3dc113561ff32466a8aa03b91c6d876) Thanks [@marckrenn](https://github.com/marckrenn)! - Store sub-core and sub-bar settings in agent-level JSON files so updates no longer overwrite user configuration. Legacy extension `settings.json` files are migrated into the new files and removed after a successful migration.

  Manual migration (if you want to do it yourself before updating):

  ```
  cp ~/.pi/agent/extensions/sub-core/settings.json ~/.pi/agent/pi-sub-core-settings.json
  cp ~/.pi/agent/extensions/sub-bar/settings.json ~/.pi/agent/pi-sub-bar-settings.json
  ```

  Existing users should move legacy settings from the extension folders to:
  - `~/.pi/agent/pi-sub-core-settings.json`
  - `~/.pi/agent/pi-sub-bar-settings.json`

- [#23](https://github.com/marckrenn/pi-sub/pull/23) [`9c324fc`](https://github.com/marckrenn/pi-sub/commit/9c324fc7daae2a874816f600ac1ea422f3799dd2) Thanks [@marckrenn](https://github.com/marckrenn)! - Auto-post theme share strings when saving, add a “Share theme” menu entry, allow share strings without a name when importing, and post raw share strings (without the `/sub-bar:import` prefix).

- [#25](https://github.com/marckrenn/pi-sub/pull/25) [`ab97c8f`](https://github.com/marckrenn/pi-sub/commit/ab97c8f13c567c32581bb82fe5b0406b3f2464ca) Thanks [@marckrenn](https://github.com/marckrenn)! - Refine the theme menu ordering/labels and add rename support for saved themes in the Manage & Load flow.

- [#27](https://github.com/marckrenn/pi-sub/pull/27) [`549c6fe`](https://github.com/marckrenn/pi-sub/commit/549c6fe3eb57374a54bfc69ad70c91862250a186) Thanks [@marckrenn](https://github.com/marckrenn)! - Move Themes to the root settings menu, rename Display Settings to Adv. Display Settings, and rename “Manage & Load themes” to “Load & Manage themes”.

- [#26](https://github.com/marckrenn/pi-sub/pull/26) [`af28d98`](https://github.com/marckrenn/pi-sub/commit/af28d9820f80bc1d045783644afbcc4d7cd114f1) Thanks [@marckrenn](https://github.com/marckrenn)! - Prompt to upload theme share strings as secret GitHub gists and post the gist URL when accepted.

- Updated dependencies [[`af0828a`](https://github.com/marckrenn/pi-sub/commit/af0828a8d2e529497a1acff95e388a0a3eabb90e), [`3e5a026`](https://github.com/marckrenn/pi-sub/commit/3e5a026ea3dc113561ff32466a8aa03b91c6d876), [`a6c0d33`](https://github.com/marckrenn/pi-sub/commit/a6c0d33c8d19d2876a4a8a1a0a69302a3c63f5e8), [`7da1e08`](https://github.com/marckrenn/pi-sub/commit/7da1e082e634f4e4dee2560b4d490527d1543ade), [`1f5e451`](https://github.com/marckrenn/pi-sub/commit/1f5e45173b9868b0d6645ae35a084142a0ac56a5), [`35eb185`](https://github.com/marckrenn/pi-sub/commit/35eb18590f369db4cda931b8e11099d0f3ddb4ec)]:
  - @marckrenn/pi-sub-core@1.0.4
  - @marckrenn/pi-sub-shared@1.0.4

## 1.0.3

### Patch Changes

- [`6fa2736`](https://github.com/marckrenn/pi-sub/commit/6fa27363573f34c38a372a6d7b8b74e756716724) Thanks [@marckrenn](https://github.com/marckrenn)! - Update extension tool execute signature order for compatibility with latest Pi API.

- Updated dependencies [[`6fa2736`](https://github.com/marckrenn/pi-sub/commit/6fa27363573f34c38a372a6d7b8b74e756716724)]:
  - @marckrenn/pi-sub-core@1.0.3
  - @marckrenn/pi-sub-shared@1.0.3

## 1.0.2

### Patch Changes

- [#3](https://github.com/marckrenn/pi-sub/pull/3) [`4ceb5ad`](https://github.com/marckrenn/pi-sub/commit/4ceb5ad133166237652d197ba9296ad1589a813c) Thanks [@marckrenn](https://github.com/marckrenn)! - Bundle sub-core with sub-bar, refresh Antigravity quotas + settings, and update UI copy/controls.

- Updated dependencies [[`4ceb5ad`](https://github.com/marckrenn/pi-sub/commit/4ceb5ad133166237652d197ba9296ad1589a813c)]:
  - @marckrenn/pi-sub-core@1.0.2
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
