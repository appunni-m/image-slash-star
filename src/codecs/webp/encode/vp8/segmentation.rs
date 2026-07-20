//! VP8 macroblock segmentation — RFC 6386 Section 9.3.
//!
//! Segmentation allows different macroblock regions of the frame to use
//! different quantizer offsets, enabling adaptive quality allocation.
//! A basic encoder can leave segmentation disabled (a single segment for
//! all macroblocks).

#![allow(dead_code)]

/// Number of segments supported (VP8 allows up to 4).
pub const NUM_SEGMENTS: usize = 4;

/// VP8 macroblock segment feature data.
///
/// When `enabled` is false, the frame uses a single quantizer for all
/// macroblocks.  When enabled, each macroblock carries a segment ID (0-3)
/// and the decoder applies per-segment feature deltas.
#[derive(Debug, Clone)]
pub struct Segmentation {
    /// Whether segmentation is active.
    pub enabled: bool,
    /// Whether the segment map is updated in this frame.
    pub update_map: bool,
    /// Per-segment quantizer offset (-127 to 127), added to the base
    /// quantizer index.
    pub quantizer_update: [i8; NUM_SEGMENTS],
    /// Whether each segment has an active quantizer offset.
    pub seg_feature_active: [bool; NUM_SEGMENTS],
}

impl Segmentation {
    /// Create a default (disabled) segmentation state.
    ///
    /// All segments have zero offset and are inactive.
    pub fn new() -> Self {
        Self {
            enabled: false,
            update_map: false,
            quantizer_update: [0i8; NUM_SEGMENTS],
            seg_feature_active: [false; NUM_SEGMENTS],
        }
    }

    /// Create a segmentation configuration from a quality hint.
    ///
    /// Lower quality values produce more aggressive segmentation differences.
    /// At the highest quality the feature is effectively disabled.
    ///
    /// The segmentation is `enabled` only when `quality < 80`.
    pub fn from_quality(quality: u8) -> Self {
        if quality >= 80 {
            // High quality — no segmentation needed.
            return Self::new();
        }

        // Lower quality → larger quantizer offsets between segments.
        // Segments 0 and 1 get negative offsets (finer quantisation),
        // segments 2 and 3 get positive offsets (coarser quantisation).
        let base_offset: i8 = ((80 - quality) / 10) as i8;
        let offset0 = -(base_offset * 2).min(127);
        let offset1 = -(base_offset).min(127).max(-127);
        let offset2 = base_offset.min(127);
        let offset3 = (base_offset * 2).min(127);

        Self {
            enabled: true,
            update_map: true,
            quantizer_update: [offset0, offset1, offset2, offset3],
            seg_feature_active: [true, true, true, true],
        }
    }

    /// Get the quantizer offset for a given segment ID (0-3).
    ///
    /// Returns 0 if segmentation is disabled or the segment has no active
    /// quantizer feature.
    pub fn quantizer_offset(&self, segment_id: u8) -> i8 {
        if !self.enabled {
            return 0;
        }
        let idx = (segment_id as usize) % NUM_SEGMENTS;
        if self.seg_feature_active[idx] {
            self.quantizer_update[idx]
        } else {
            0
        }
    }
}

impl Default for Segmentation {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
