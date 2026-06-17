//! Shared scene framework boundary.
//!
//! This module is reserved for reusable scene capabilities:
//! scene lifecycle, scene resource loading, common camera setup, and
//! generic entity organization helpers.
//!
//! Concrete gameplay scenes, level content, feature state, and screen-specific
//! UI stay in the game layer, usually in `game/features` or `game/screens`.
//! Code in this module must not depend on the game layer.
