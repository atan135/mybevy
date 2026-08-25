# MyServer Shared Code Snapshot

This directory vendors the MyServer code required to build the client without a
second repository checkout:

- `authority-core/`
- `sim-core/`
- `proto/game.proto`
- `proto/chat.proto`

Source repository: `atan135/myserver`
Source commit: `13b1d9e019553400ea94cbf5d69f5ff4bbdb9677`

The vendored `game.proto` hash is `b5b475cdedad2c97c18d7d68e912a200e3fca17d`.
This snapshot includes the player activity contract (`1435-1446`), including
the character-bound claim-history query (`1445/1446`).

When a client-visible authority contract, deterministic simulation rule, or
generated game/chat protocol changes in MyServer, update the affected snapshot
files here in the same integration change and record the new source commit.
