# Console playtime and presence: PlayStation, Xbox, Switch

Researched 2026-11-02 from library source and docs. Nothing was authenticated and no endpoint was called. Everything below is an unofficial, reverse-engineered API (no public partner API for consumers on any of the three).

## 1. PlayStation

**Auth (NPSSO to tokens)**, from [psn-api `authenticate/`](https://github.com/achievements-app/psn-api/tree/main/src/authenticate) and [psnawp `authenticator.py`](https://codeberg.org/YoshikageKira/psnawp/src/branch/master/src/psnawp_api/core/authenticator.py):

- The user signs in at playstation.com, then opens `https://ca.account.sony.com/api/v1/ssocookie` and copies the 64-char `npsso`. They paste it into the app. This is the only manual step.
- `GET ca.account.sony.com/api/authz/v3/oauth/authorize` with `Cookie: npsso=...`, `client_id=09515159-7237-4370-9b40-3806e67c0891` (the PlayStation Android app), `scope=psn:mobile.v2.core psn:clientapp`, `access_type=offline`, `redirect_uri=com.scee.psxandroid.scecompcall://redirect`. Do not follow redirects. The 302 `Location` header carries `?code=v3.xxx`.
- `POST .../oauth/token`, Basic auth with the app's client secret, `grant_type=authorization_code`, `token_format=jwt`. The result has `access_token`, `expires_in`, `refresh_token`, `refresh_token_expires_in`.
- Refresh with `grant_type=refresh_token`. Access tokens are short (about an hour; the code reads `expires_in`, it does not hardcode it). The refresh token lasts about 2 months ([psnawp README](https://codeberg.org/YoshikageKira/psnawp), [Home Assistant](https://www.home-assistant.io/integrations/playstation_network)). Store the refresh token. After about 60 days the user must paste a fresh NPSSO. Warn at 3 days left, as psnawp does.
- Logging out of playstation.com can invalidate the NPSSO. Generating a new NPSSO invalidates the previous one (psnawp README).

**Playtime per title**: `GET https://m.np.playstation.com/api/gamelist/v2/users/me/titles` (also `/{accountId}/titles`).

- Fields ([model](https://github.com/achievements-app/psn-api/blob/main/src/models/user-played-games-response.model.ts)): `titleId` (e.g. `PPSA01521_00`), `name`, `localizedName`, `imageUrl`, `category` (`ps4_game`, `ps5_native_game`, `pspc_game`, `unknown`), `service`, `playCount`, `firstPlayedDateTime`, `lastPlayedDateTime` (ISO 8601), `playDuration`, `concept{id, titleIds, name, media.images[]}`.
- `playDuration` is an ISO-8601 duration with hours as the largest unit, such as `PT228H56M33S`. Per psnawp it can be missing or malformed, so parse leniently (`PT(\d+H)?(\d+M)?(\d+S)?`).
- Pagination: `limit` and `offset` query parameters; the response gives `totalItemCount`, `nextOffset`, `previousOffset`. `categories=ps4_game,ps5_native_game,pspc_game` filters.
- Gaps: lifetime totals only, no per-session history (derive sessions from presence plus `playDuration` deltas); PS3, Vita and streamed games are absent; another user's list is subject to privacy settings. A PS5 disc-game blank-playtime bug is reported on social media only (unverified).

**Presence**: `GET https://m.np.playstation.com/api/userProfile/v2/internal/users/{accountId}/basicPresences?type=primary&platforms=PS4,PS5,MOBILE_APP,PSPC&withOwnGameTitleInfo=true` ([psnawp `user.py`](https://codeberg.org/YoshikageKira/psnawp/src/branch/master/src/psnawp_api/models/user.py)). The v1 form is in [psn-api `getBasicPresence`](https://github.com/achievements-app/psn-api/blob/main/src/user/getBasicPresence.ts).

- Response: `basicPresence.availability`, `primaryPlatformInfo{onlineStatus, platform, lastOnlineDate}`, and `gameTitleInfoList[{npTitleId, titleName, format, launchPlatform, npTitleIconUrl, conceptIconUrl}]`. A non-empty `gameTitleInfoList` means playing.
- Privacy: a 403 means the account hides its online status ([psnawp raises `PSNAWPForbiddenError`](https://codeberg.org/YoshikageKira/psnawp)). "Appear Offline" and "Busy" are Sony account settings ([support](https://www.playstation.com/en-us/support/account/change-online-status)). Neither library has a workaround. Treat as offline and surface "presence hidden" in the UI.
- Polling: Home Assistant polls every 30 s with 3 requests per cycle and documents Sony's limit of 300 requests per 15 min. 30 to 60 s is sensible; back off on 429.

**Art and ids**: `imageUrl` and `concept.media.images[]` are on `image.api.playstation.com`. Presence `npTitleId` matches `titleId`. Use `concept.id` to merge PS4 and PS5 versions of one game.

## 2. Xbox

**Auth**: two options.

- _Direct_ ([xbox-webapi-python `authentication/manager.py`](https://github.com/OpenXbox/xbox-webapi-python/blob/master/xbox/webapi/authentication/manager.py)):
  1. Microsoft OAuth2 at `login.live.com/oauth20_authorize.srf`, scopes `Xboxlive.signin Xboxlive.offline_access`, then the code goes to `oauth20_token.srf`.
  2. `POST user.auth.xboxlive.com/user/authenticate` with `RpsTicket=d=<access_token>` gives the user token (XAU).
  3. `POST xsts.auth.xboxlive.com/xsts/authorize` with `RelyingParty=http://xboxlive.com` gives the XSTS token. Send it as `Authorization: XBL3.0 x=<uhs>;<token>`.
  4. Refresh the Microsoft token with `refresh_token`, then redo XAU and XSTS (hours-scale lifetimes).
  - Needs a registered Azure app: "Personal Microsoft accounts only", loopback redirect (python-xbox uses `http://localhost/auth/callback`) and a client secret. [python-xbox](https://github.com/tr4nt0r/python-xbox) documents this. A desktop app should use the public-client PKCE variant instead of shipping a secret; this is **unverified**.
  - The account must be **non-child (18+)**; the XSTS call returns 401 otherwise.
  - The lib's newer `xal.py` impersonates first-party Xbox app IDs with request signing. Avoid it.
- _OpenXBL_ ([xbl.io](https://xbl.io)): hosted gateway, free tier 150 requests/hour, phone-verified key, user data passes through a third party. **Not recommended** for a local app. Use direct auth with our own Azure app, as [Home Assistant](https://www.home-assistant.io/integrations/xbox/) does.

**Playtime per title**:

- `GET https://titlehub.xboxlive.com/users/xuid({xuid})/titles/titlehistory/decoration/scid,image,achievement` (`x-xbl-contract-version: 2`, `maxItems`) lists titles with `titleId`, `pfn`, `serviceConfigId`, `name`, `displayImage`, `images[]` and `titleHistory.lastTimePlayed`. **It has no playtime.**
- Playtime is `GET https://userstats.xboxlive.com/users/xuid({xuid})/scids/{scid}/stats/MinutesPlayed` (contract version 2). A batch `POST /batch` with `groups` and `stats` takes many xuids per scid ([source](https://github.com/OpenXbox/xbox-webapi-python/blob/master/xbox/webapi/api/provider/userstats/__init__.py)). The value is **integer minutes**, lifetime, per scid.
- Gaps: the stat is **publisher-defined**. Xbox's own help says "Not all games track this stat" ([support](https://support.xbox.com/en-US/help/games-apps/my-games-apps/time-played)). Coverage is weaker for older Xbox 360, Windows-store and cloud titles. Stats can lag after play. Each title needs its own request (the scid comes from titlehub). The lib's rate limits are 100 burst and 300 sustain, with some Microsoft per-title throttling.
- Treat a missing stat as "no data", not zero.

**Presence**: `GET https://userpresence.xboxlive.com/users/xuid({xuid})?level=all` or `/users/me`, contract version 3 ([source](https://github.com/OpenXbox/xbox-webapi-python/blob/master/xbox/webapi/api/provider/presence/__init__.py)).

- Fields: `state` (Online or Offline), `lastSeen{titleId, titleName, deviceType, timestamp}`, `devices[].titles[]{id, name, placement, state, activity[].richPresence}`.
- The title with `placement: Full` and a game type is the foreground game. Freshness is not documented (unverified).
- Privacy: Xbox has an Appear Offline mode (`PresenceState.CLOAKED` in the lib). Whether `/users/me` reveals your own true state while cloaked is unverified.
- Polling: Home Assistant polls presence every 30 s and console status every 10 s. 30 to 60 s is fine.

**Art and ids**: `titleId` is the cross-endpoint key; `scid` is needed for stats. Art comes from titlehub `images[]` and `displayImage`.

## 3. Nintendo Switch

**Two separate apps and APIs**, per [nxapi](https://github.com/samuelthomas2774/nxapi) (TypeScript, **AGPL-3.0**).

- **Parental Controls (the realistic source of hours)**: Nintendo Account OAuth with PKCE, `client_id=54789befb391a838`, `redirect_uri=npf54789befb391a838://auth`, `response_type=session_token_code`, using the Moon scopes (`moonParentalControlSetting*`, `moonDevice:create`, ...). The user signs in at accounts.nintendo.com, copies the redirect link (the browser cannot open the `npf...` scheme), and the app exchanges it at `accounts.nintendo.com/connect/1.0.0/api/session_token`. The long-lived **session token** then mints access tokens (about 15 min) via `.../api/token` with `grant_type=...jwt-bearer-session-token`, and the app calls `https://api-lp1.pctl.srv.nintendo.net/moon` ([`moon.ts`](https://github.com/samuelthomas2774/nxapi/blob/main/src/api/moon.ts), [`na.ts`](https://github.com/samuelthomas2774/nxapi/blob/main/src/api/na.ts)). Send `X-Moon-*` headers that mimic the app. This path needs no third-party service.
- **Nintendo Switch Online app (Coral)**: the same session-token idea, but the app login needs an `f` value generated by Nintendo's native library. nxapi sends the id_token to a **third-party service** (`nxapi-znca-api.fancy.org.uk`), which requires registered nxapi-auth clients since June 2025 ([README](https://github.com/samuelthomas2774/nxapi#coral-client-authentication)). This is the largest dependency risk of the whole project.

**Playtime**: `GET /v1/users/{naId}/devices` then `GET /v1/devices/{id}/daily_summaries` and `/monthly_summaries[/{yyyy-MM}]`.

- Daily summary fields ([`moon-types.ts`](https://github.com/samuelthomas2774/nxapi/blob/main/src/api/moon-types.ts)): `date`, `playingTime` (**seconds**; the CLI divides by 60), `playedApps[]` (applicationId, title, imageUri, shopUri, `firstPlayDate`) and `devicePlayers[].playedApps[]{applicationId, firstPlayDate, playingTime}` for per-title seconds per player per day.
- This is **per-day per-title time**, a better shape for session reconstruction than the other two. It needs:
  - a console paired in the Parental Controls app, with play-time logging enabled;
  - the console to have synced (the response carries `updatedRecently`);
  - the user to own the Nintendo Account that is the parent or owner.
- It only covers history since pairing, with a limited retention window. The window length is unverified. No Switch lifetime hours exist for earlier play.

**Presence**: the Coral friend list (`getFriendList`, v4 types in [`coral-types.ts`](https://github.com/samuelthomas2774/nxapi/blob/main/src/api/coral-types.ts)) returns `presence{state: OFFLINE|INACTIVE|ONLINE|PLAYING, updatedAt, logoutAt, game{name, imageUri, shopUri, totalPlayTime, firstPlayedAt}}`. `totalPlayTime` is a lifetime total per friend per game (unit not documented in the types; likely minutes), which is the only lifetime-hours source for the Switch, but only for friends.

- **A user's own presence is no longer available** from the API (nxapi issue [#1](https://github.com/samuelthomas2774/nxapi/issues/1)). nxapi's own README says a **secondary Nintendo account** must be friended with the main one, and the secondary signs in to Coral to see the main account's presence.
- Presence is hidden if the friend has turned it off in Nintendo's settings. "Appear offline" is not a separate mode.
- `updatedAt` refreshes about hourly even when nothing changes, and nxapi's monitors poll about every 60 s.

**Art and ids**: `applicationId` (a 16-hex title id) on Parental Controls; Coral uses a numeric `game.id` or an EC url (`getTitleIdFromEcUrl`). `imageUri` and `shopUri` give art. Mapping Coral ids to Parental Controls ids is unverified.

## 4. Libraries

| Platform | Library                                                              | Lang   | Licence      | State (Nov 2026)                       | Borrow                                                                                  |
| -------- | -------------------------------------------------------------------- | ------ | ------------ | -------------------------------------- | --------------------------------------------------------------------------------------- |
| PSN      | [psn-api](https://github.com/achievements-app/psn-api)               | TS     | MIT          | pushed 2026-08, 432 stars, active      | the 3 auth calls, gamelist and presence shapes, no deps                                 |
| PSN      | [psnawp](https://codeberg.org/YoshikageKira/psnawp)                  | Python | MIT          | moved to Codeberg, commits 2026-10     | `play_duration_to_timedelta`, rate limiter, the error taxonomy                          |
| Xbox     | [xbox-webapi-python](https://github.com/OpenXbox/xbox-webapi-python) | Python | MIT          | last commit 2026-08                    | XBL3.0 header, userstats, titlehub, presence models                                     |
| Xbox     | [python-xbox](https://github.com/tr4nt0r/python-xbox)                | Python | MIT          | active 2026-10, used by Home Assistant | a fork that tracks current endpoints                                                    |
| Switch   | [nxapi](https://github.com/samuelthomas2774/nxapi)                   | TS     | **AGPL-3.0** | pushed 2026-09, 41 open issues         | **read it, do not copy it**; reimplement the Moon calls clean-room from the HTTP shapes |

Gamelapse is open source, but check licence compatibility before porting anything from nxapi.

## 5. Risks

- **ToS**: all three APIs are undocumented. Sony and Nintendo can in principle suspend accounts. Both libraries warn of this: psnawp says "temporarily or permanently banned", nxapi says "extremely unlikely" and notes a small number of SplatNet 3 bans in 2023. Microsoft's data is legitimate Xbox Live REST, but the first-party spoofing (XAL) route is not. Prefer our own Azure app.
- **Rate limits**: PSN 300 per 15 min (psnawp self-limits at 1 per 3 s). Xbox has per-user and per-title burst (15 s) and sustain (5 min) windows. Nintendo has no published limits; nxapi polls every 60 s.
- **Breakage history**: Nintendo's Coral attestation changed repeatedly (HMAC, native library, private service), so the Switch is the most fragile. PSN GraphQL hashes rotate; REST is steadier. Xbox contract versions are stable.
- **Appear offline**: no library sees through it on any platform (PSN 403 or offline, Xbox cloaked gives Offline, Switch OFFLINE). Fall back to lifetime-total deltas (`playDuration`, `MinutesPlayed`, daily summaries) so playtime is still recorded.
- **Secrets**: NPSSO, refresh tokens and session tokens equal passwords. Keep them in Windows Credential Manager (Rust `keyring` crate), not in SQLite or logs.

## 6. Recommendation (Rust, effort in focused days)

Order by value for effort and by risk. All estimates assume `reqwest` plus `serde`, tokens in the Windows credential store, a polling task in the existing tracker, and import through the existing importer shape.

1. **PlayStation, 4 to 5 days.** Best ratio. Setup is one paste of NPSSO every ~60 days. Days: auth plus refresh (1), gamelist hours import with pagination and duration parser (1), presence poller to sessions with 403 handling (1.5), settings UI, expiry warning and tests (1 to 1.5). Session approach: poll every 30 to 60 s, open a session when `gameTitleInfoList` is non-empty, close on empty, and reconcile against `playDuration` deltas.
2. **Xbox, 5 to 7 days.** Setup: sign in once in the browser (our Azure app, no secrets for the user); an 18+ account. Days: register the app plus PKCE loopback flow (1), XAU and XSTS chain with refresh (1.5), titlehub plus batched userstats import (1.5), presence poller (1), UI, error cases and tests (1 to 2). Expect partial coverage; label titles "no playtime reported".
3. **Switch, 8 to 12 days, optional and last.** Setup is the worst of the three: a manual link copy-paste for the session token, a console paired in the Parental Controls app, and for live presence a second Nintendo account plus friend request. Days: PKCE session-token flow with the manual redirect paste (2), Moon token mint and devices (1.5), daily-summary import (2), clean-room schema work and tests (2 to 3), optional Coral presence behind the third-party `f` service (3 to 4). **Ship Parental Controls hours only and skip Coral presence in v1.**

Cross-cutting (about 2 days, shared): a `ConsoleAccount` abstraction (provider, account id, token ref, last sync), a session-from-presence state machine with a grace period (about 2 missed polls), and an "unofficial API" consent screen on first connect.

Still unverified: exact PSN token lifetimes, whether Microsoft allows a public PKCE client for `Xboxlive.signin`, the Parental Controls retention window, and Coral-to-Moon title-id mapping.
