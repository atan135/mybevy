# MyServer Shared Code Snapshot

This directory vendors the MyServer code required to build the client without a
second repository checkout:

- `authority-core/`
- `sim-core/`
- `proto/game.proto`
- `proto/game_item.proto`
- `proto/game_activity.proto`
- `proto/chat.proto`

Source repository: `atan135/myserver`
Source commit (unchanged shared crates and other protocols): `ae7c6f3adafcd364530ae4b27edb59497506da44`

`proto/game.proto` source commit: `aaf7eb6de9883cddd5631dc8bd69fa3c3d51a587`

The vendored `game.proto` hash is `ce1b6c741ffcaf219c0e43357aa3dc9ffef47c22`.
The vendored `game_item.proto` hash is `9dad1286ed0ec830d813cb1c71fe48ad68820dee`.
The vendored `game_activity.proto` hash is `9365c09924f3bcc039fe06aef72da7c46dfa3326`.
This snapshot includes the player activity contract (`1435-1446`), including
the character-bound claim-history query (`1445/1446`).

When a client-visible authority contract, deterministic simulation rule, or
generated game/chat protocol changes in MyServer, update the affected snapshot
files here in the same integration change and record the new source commit.

The game protocol includes `EntityTransform.speed_metres_per_second` (field 10),
with application protocol version 2 required by MyServer. The shared crates and
other proto files were unchanged by this integration. Zero/omitted speed remains
the protobuf default and must be rejected by the movement consumer; it is not a
fallback movement speed.
