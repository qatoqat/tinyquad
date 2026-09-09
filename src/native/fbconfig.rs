//! Framebuffer-config selection shared by the X11/GLX and
//! Windows/WGL context-creation backends.
//!
//! The heuristic is GLFW's, kept verbatim; it used to exist as two
//! diverging copies (one of which had an accumulation bug) — it now
//! lives here exactly once, as pure, unit-tested logic.

/// One framebuffer-format description. `-1` means "don't care".
pub(crate) struct FbConfigSpec {
    pub red_bits: i32,
    pub green_bits: i32,
    pub blue_bits: i32,
    pub alpha_bits: i32,
    pub depth_bits: i32,
    pub stencil_bits: i32,
    pub samples: i32,
    pub doublebuffer: bool,
}

/// Pick the index of the best-matching config: fewest missing
/// features first, then the closest color channel sizes, then the
/// closest alpha/depth/stencil/sample counts.
pub(crate) fn fbconfig_choose(
    desired: &FbConfigSpec,
    alternatives: &[FbConfigSpec],
) -> Option<usize> {
    let mut least_missing: i32 = 1_000_000;
    let mut least_color_diff: i32 = 10_000_000;
    let mut least_extra_diff: i32 = 10_000_000;
    let mut closest = None;

    for (i, current) in alternatives.iter().enumerate() {
        if desired.doublebuffer != current.doublebuffer {
            continue;
        }

        let mut missing = 0;
        if desired.alpha_bits > 0 && current.alpha_bits == 0 {
            missing += 1;
        }
        if desired.depth_bits > 0 && current.depth_bits == 0 {
            missing += 1;
        }
        if desired.stencil_bits > 0 && current.stencil_bits == 0 {
            missing += 1;
        }
        if desired.samples > 0 && current.samples == 0 {
            // Technically, several multisampling buffers could be
            // involved, but that's a lower level implementation detail
            // and not important to us here, so we count them as one
            missing += 1;
        }

        // These polynomials make many small channel size differences
        // matter less than one large channel size difference.
        let sq = |a: i32, b: i32| (a - b) * (a - b);

        let mut color_diff = 0;
        if desired.red_bits != -1 {
            color_diff += sq(desired.red_bits, current.red_bits);
        }
        if desired.green_bits != -1 {
            color_diff += sq(desired.green_bits, current.green_bits);
        }
        if desired.blue_bits != -1 {
            color_diff += sq(desired.blue_bits, current.blue_bits);
        }

        let mut extra_diff = 0;
        if desired.alpha_bits != -1 {
            extra_diff += sq(desired.alpha_bits, current.alpha_bits);
        }
        if desired.depth_bits != -1 {
            extra_diff += sq(desired.depth_bits, current.depth_bits);
        }
        if desired.stencil_bits != -1 {
            extra_diff += sq(desired.stencil_bits, current.stencil_bits);
        }
        if desired.samples != -1 {
            extra_diff += sq(desired.samples, current.samples);
        }

        if missing < least_missing
            || missing == least_missing
                && (color_diff < least_color_diff
                    || color_diff == least_color_diff && extra_diff < least_extra_diff)
        {
            closest = Some(i);
        }

        if closest == Some(i) {
            least_missing = missing;
            least_color_diff = color_diff;
            least_extra_diff = extra_diff;
        }
    }
    closest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn spec(
        red: i32,
        green: i32,
        blue: i32,
        alpha: i32,
        depth: i32,
        stencil: i32,
        samples: i32,
        doublebuffer: bool,
    ) -> FbConfigSpec {
        FbConfigSpec {
            red_bits: red,
            green_bits: green,
            blue_bits: blue,
            alpha_bits: alpha,
            depth_bits: depth,
            stencil_bits: stencil,
            samples,
            doublebuffer,
        }
    }

    const RGB: (i32, i32, i32) = (8, 8, 8);

    #[test]
    fn picks_exact_match() {
        let desired = spec(RGB.0, RGB.1, RGB.2, 8, 24, 8, -1, true);
        let alts = [
            spec(RGB.0, RGB.1, RGB.2, 8, 24, 0, -1, true),
            spec(RGB.0, RGB.1, RGB.2, 8, 24, 8, -1, true),
        ];
        assert_eq!(fbconfig_choose(&desired, &alts), Some(1));
    }

    #[test]
    fn empty_alternatives_yield_none() {
        let desired = spec(RGB.0, RGB.1, RGB.2, 8, 24, 8, -1, true);
        assert_eq!(fbconfig_choose(&desired, &[]), None);
    }

    #[test]
    fn doublebuffer_mismatch_is_skipped_even_if_exact() {
        let desired = spec(RGB.0, RGB.1, RGB.2, 8, 24, 8, -1, true);
        let alts = [
            spec(RGB.0, RGB.1, RGB.2, 8, 24, 8, -1, false),
            spec(RGB.0, RGB.1, RGB.2, 4, 16, 0, -1, true),
        ];
        assert_eq!(fbconfig_choose(&desired, &alts), Some(1));
    }

    #[test]
    fn missing_features_dominate_color_closeness() {
        // A config with the wanted depth beats one without it, even
        // though the other's color channels are a perfect match.
        let desired = spec(RGB.0, RGB.1, RGB.2, -1, 24, -1, -1, true);
        let alts = [
            spec(RGB.0, RGB.1, RGB.2, -1, 0, -1, -1, true),
            spec(4, 4, 4, -1, 24, -1, -1, true),
        ];
        assert_eq!(fbconfig_choose(&desired, &alts), Some(1));
    }

    #[test]
    fn color_diff_dominates_extra_diff() {
        // Perfect alpha but off-red loses to perfect red but off
        // alpha: color closeness outranks the alpha/depth/stencil
        // bucket.
        let desired = spec(8, -1, -1, 8, -1, -1, -1, true);
        let alts = [
            spec(6, -1, -1, 8, -1, -1, -1, true),
            spec(8, -1, -1, 4, -1, -1, -1, true),
        ];
        assert_eq!(fbconfig_choose(&desired, &alts), Some(1));
    }

    /// Regression test: the stencil clause used to *assign*
    /// `extra_diff` instead of accumulating into it (discarding the
    /// alpha and depth contributions). With that bug, this picks
    /// candidate 1; with correct accumulation, the two tie at 16 and
    /// the first candidate wins.
    #[test]
    fn extra_diff_accumulates_alpha_depth_and_stencil() {
        let desired = spec(-1, -1, -1, 8, 24, 8, -1, true);
        let alts = [
            // extra = (8-8)^2 + (24-24)^2 + (8-4)^2 = 16
            spec(-1, -1, -1, 8, 24, 4, -1, true),
            // extra = (8-4)^2 + (24-24)^2 + (8-6)^2 = 16 + 4 = 20
            spec(-1, -1, -1, 4, 24, 6, -1, true),
        ];
        assert_eq!(fbconfig_choose(&desired, &alts), Some(0));
    }
}
