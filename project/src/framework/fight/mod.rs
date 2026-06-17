//! Shared fight framework boundary.
//!
//! This module is reserved for reusable combat foundations:
//! battle lifecycle, combat events, state machines, and generic skill/effect
//! interfaces.
//!
//! Concrete characters, numeric tuning, feature-specific rules, and
//! screen-specific combat UI stay in the game layer, usually in `game/features`
//! or `game/screens`. Code in this module must not depend on the game layer.
