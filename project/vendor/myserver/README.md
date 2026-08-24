# MyServer Shared Code Snapshot

This directory vendors the MyServer code required to build the client without a
second repository checkout:

- `authority-core/`
- `sim-core/`
- `proto/game.proto`
- `proto/chat.proto`

Source repository: `atan135/myserver`
Source commit: `f61fbc98f02f1cfac48a1ba11ebdd73e324ad1ae`

The vendored `game.proto` hash is `297a8e226a1b4414545b7a39770d96581f10b076`.
This snapshot includes the player activity contract (`1435-1444`); the
character-bound claim-history query is intentionally not included because the
server has not assigned a message type for it yet.

When a client-visible authority contract, deterministic simulation rule, or
generated game/chat protocol changes in MyServer, update the affected snapshot
files here in the same integration change and record the new source commit.
