# Security policy

## Reporting a vulnerability

Please do not open a public issue. Use GitHub private vulnerability reporting:

1. Go to the [Security tab](https://github.com/furkanyolal/gamelapse/security) of the repository.
2. Click **Report a vulnerability**.

Include the Gamelapse version, steps to reproduce and the impact you see. This is a volunteer project, so there is no guaranteed response time, but reports are read and taken seriously.

## Scope

In scope:

- The Gamelapse application and installer
- The update mechanism
- Handling of files the app reads, such as launcher manifests and restored backups

Out of scope:

- Vulnerabilities in Windows, WebView2, Steam, Epic Games or GOG Galaxy
- Issues that need an attacker who already has access to your user account or files

## Data and secrets

Gamelapse stores its data locally and has no telemetry. Your database and settings live under `%APPDATA%\com.furkanyolal.gamelapse`.

If you use the optional Steam Web API import, the API key is stored in plain text in `settings.json` in that folder. Anyone who can read your user files can read it. Do not share the file or your backups of it, and revoke the key at <https://steamcommunity.com/dev/apikey> if it leaks.
