# Ourea Desktop

Ourea Desktop is a lightweight Tauri desktop shell for an existing Ourea Web workspace.

The desktop shell focuses on a few core desktop workflows:

- save multiple Ourea Web addresses and switch between them quickly;
- validate a target before opening it;
- keep a local fallback page when the workspace is unreachable;
- keep settings and the Ourea workspace in one window instead of splitting them across macOS windows;
- send external links to the system browser;
- remember the desktop window size and position across launches;
- build native desktop packages with Tauri for macOS, Windows, and Linux.

## Development

Prerequisites:

- Node.js 22+
- Rust stable
- Tauri Linux/macOS/Windows prerequisites for your target platform

Install dependencies:

```bash
npm install
```

Run the local desktop shell UI:

```bash
npm run dev
```

Run the Tauri desktop app:

```bash
npm run tauri:dev
```

Build the frontend:

```bash
npm run build
```

Build desktop packages:

```bash
npm run tauri:build
```

## Default Ourea URL

The launcher defaults to:

```text
http://127.0.0.1:8008
```

You can enter any `http://` or `https://` Ourea Web address. If the scheme is omitted, the app assumes `https://`.

## Release workflow

GitHub Actions builds release artifacts when a version tag is pushed:

```bash
git tag v0.1.0
git push origin v0.1.0
```
