pub const TOPIC_WEIGHT: f32 = 0.8;
pub const SEMANTIC_WEIGHT: f32 = 0.20;
pub const SCORE_THRESHOLD: f32 = 0.50;

#[derive(Debug, Clone, Default)]
pub struct ScoreBreakdown {
    pub classification_score: f32,
    pub semantic_score: f32,
    pub final_score: f32,
}

impl ScoreBreakdown {
    pub fn passes_threshold(&self) -> bool {
        self.final_score >= SCORE_THRESHOLD
    }
}

pub fn calculate_score(classification_score: f32, semantic_score: f32) -> ScoreBreakdown {
    let final_score =
        classification_score * TOPIC_WEIGHT + semantic_score * SEMANTIC_WEIGHT;

    ScoreBreakdown {
        classification_score,
        semantic_score,
        final_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_calculation() {
        let score = calculate_score(1.0, 1.0);
        let expected = TOPIC_WEIGHT + SEMANTIC_WEIGHT;
        assert!((score.final_score - expected).abs() < 0.01);
    }

    #[test]
    fn test_passes_threshold() {
        let passing = calculate_score(1.0, 1.0);
        assert!(passing.passes_threshold());

        let failing = calculate_score(0.3, 0.3);
        assert!(!failing.passes_threshold());
    }

    #[test]
    fn test_threshold_boundary() {
        let at_threshold = ScoreBreakdown {
            classification_score: 0.0,
            semantic_score: 0.0,
            final_score: SCORE_THRESHOLD,
        };
        assert!(at_threshold.passes_threshold());

        let below_threshold = ScoreBreakdown {
            classification_score: 0.0,
            semantic_score: 0.0,
            final_score: SCORE_THRESHOLD - 0.01,
        };
        assert!(!below_threshold.passes_threshold());
    }
}
