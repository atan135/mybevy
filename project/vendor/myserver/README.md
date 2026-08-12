# MyServer Shared Code Snapshot

This directory vendors the MyServer code required to build the client without a
second repository checkout:

- `authority-core/`
- `sim-core/`
- `proto/game.proto`
- `proto/chat.proto`

Source repository: `atan135/myserver`
Source commit: `811a6ba05c3c3d026edc5e6790d523c688104cd5`

When a client-visible authority contract, deterministic simulation rule, or
generated game/chat protocol changes in MyServer, update the affected snapshot
files here in the same integration change and record the new source commit.
