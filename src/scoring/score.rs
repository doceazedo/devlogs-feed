use crate::settings::settings;

#[derive(Debug, Clone, Default)]
pub struct ScoreBreakdown {
    pub classification_score: f32,
    pub semantic_score: f32,
    pub final_score: f32,
}

impl ScoreBreakdown {
    pub fn passes_threshold(&self) -> bool {
        self.final_score >= settings().scoring.thresholds.score
    }
}

pub fn calculate_score(classification_score: f32, semantic_score: f32) -> ScoreBreakdown {
    let s = settings();
    let final_score = classification_score * s.scoring.weights.topic
        + semantic_score * s.scoring.weights.semantic;

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
        let s = settings();
        let score = calculate_score(1.0, 1.0);
        let expected = s.scoring.weights.topic + s.scoring.weights.semantic;
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
        let s = settings();
        let at_threshold = ScoreBreakdown {
            classification_score: 0.0,
            semantic_score: 0.0,
            final_score: s.scoring.thresholds.score,
        };
        assert!(at_threshold.passes_threshold());

        let below_threshold = ScoreBreakdown {
            classification_score: 0.0,
            semantic_score: 0.0,
            final_score: s.scoring.thresholds.score - 0.01,
        };
        assert!(!below_threshold.passes_threshold());
    }
}
