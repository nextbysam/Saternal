/// Layout padding constants shared across rendering and terminal sizing
///
/// These values define the margins around terminal content and must be synchronized
/// between:
/// - Terminal size calculations (to determine PTY dimensions)
/// - Text rasterization (to position glyphs on screen)
///
/// Changing these values will affect both the visual layout and the calculated
/// terminal dimensions (cols/rows).

/// Left padding in pixels
pub const PADDING_LEFT: f32 = 10.0;

/// Top padding in pixels
pub const PADDING_TOP: f32 = 5.0;

/// Right padding in pixels
pub const PADDING_RIGHT: f32 = 10.0;

/// Bottom padding in pixels to ensure bottom line is visible
pub const PADDING_BOTTOM: f32 = 10.0;

/// Minimum cell dimension to prevent division by zero
/// Used as a fallback when cell dimensions are invalid
pub const MIN_CELL_DIMENSION: f32 = 1.0;
