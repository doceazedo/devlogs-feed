use anyhow::Result;
use rust_bert::pipelines::zero_shot_classification::ZeroShotClassificationModel;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;
use strum::{Display, EnumIter, IntoEnumIterator, IntoStaticStr};

use super::semantic::{compute_reference_embeddings, semantic_similarity};

pub const WEIGHT_CLASSIFICATION: f32 = 0.50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter, IntoStaticStr)]
pub enum TopicLabel {
    #[strum(to_string = "game developer sharing their own work")]
    GameDevSharingWork,
    #[strum(to_string = "game programming or technical development")]
    GameProgramming,
    #[strum(to_string = "gamer discussing games they play")]
    GamerDiscussing,
    #[strum(to_string = "game review or gameplay opinion")]
    GameReview,
    #[strum(to_string = "marketing or promotional content")]
    Marketing,
    #[strum(to_string = "job posting or job search")]
    JobPosting,
    #[strum(to_string = "AI generated")]
    GenAi,
    #[strum(to_string = "crypto or NFT related")]
    CryptoNFT,
    #[strum(to_string = "unrelated")]
    Unrelated,
}

impl TopicLabel {
    pub fn is_positive(&self) -> bool {
        self.multiplier() >= 1.0
    }

    pub fn multiplier(&self) -> f32 {
        match self {
            Self::GameDevSharingWork => 2.0,
            Self::GameProgramming => 1.2,
            Self::GamerDiscussing => 0.8,
            Self::GameReview => 0.6,
            Self::Marketing => 0.2,
            Self::JobPosting => 0.2,
            Self::GenAi => 0.0,
            Self::CryptoNFT => 0.0,
            Self::Unrelated => 0.2,
        }
    }

    pub fn positive_labels() -> Vec<&'static str> {
        Self::iter()
            .filter(|l| l.is_positive())
            .map(|l| l.into())
            .collect()
    }

    pub fn negative_labels() -> Vec<&'static str> {
        Self::iter()
            .filter(|l| !l.is_positive())
            .map(|l| l.into())
            .collect()
    }

    pub fn all_labels() -> Vec<&'static str> {
        Self::iter().map(|l| l.into()).collect()
    }
}

impl FromStr for TopicLabel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::iter().find(|l| l.to_string() == s).ok_or(())
    }
}

pub fn label_multiplier(label: &str) -> f32 {
    TopicLabel::from_str(label)
        .map(|l| l.multiplier())
        .unwrap_or(0.6)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter, IntoStaticStr)]
pub enum QualityLabel {
    #[strum(to_string = "casual and personal")]
    Authentic,
    #[strum(to_string = "engagement bait or a call to action")]
    EngagementBait,
    #[strum(to_string = "templated")]
    Synthetic,
}

impl QualityLabel {
    pub fn all_labels() -> Vec<&'static str> {
        Self::iter().map(|l| l.into()).collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TopicClassification {
    pub score: f32,
    pub best_label: String,
    pub best_label_score: f32,
    pub all_labels: Vec<(String, f32)>,
    pub is_negative_label: bool,
}

#[derive(Debug, Clone, Default)]
pub struct QualityAssessment {
    pub engagement_bait_score: f32,
    pub synthetic_score: f32,
}

pub enum MLRequest {
    Score {
        text: String,
        response_tx: tokio::sync::oneshot::Sender<MLScores>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct MLScores {
    pub topic: TopicClassification,
    pub quality: QualityAssessment,
    pub semantic_score: f32,
    pub best_reference_idx: usize,

    pub classification_score: f32,
    pub best_label: String,
    pub best_label_score: f32,
    pub all_labels: Vec<(String, f32)>,
    pub is_negative_label: bool,
    pub negative_rejection: bool,
}

#[derive(Clone)]
pub struct MLHandle {
    request_tx: mpsc::Sender<MLRequest>,
}

impl MLHandle {
    pub fn spawn() -> Result<Self> {
        let (request_tx, request_rx) = mpsc::channel::<MLRequest>();

        thread::spawn(move || {
            let _ = run_ml_worker(request_rx);
        });

        Ok(Self { request_tx })
    }

    pub async fn score(&self, text: String) -> MLScores {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        if self
            .request_tx
            .send(MLRequest::Score { text, response_tx })
            .is_err()
        {
            return MLScores::default();
        }

        response_rx.await.unwrap_or_default()
    }
}

fn run_ml_worker(request_rx: mpsc::Receiver<MLRequest>) -> Result<()> {
    let classifier = ZeroShotClassificationModel::new(Default::default())?;
    let (embeddings, reference_embeddings) = compute_reference_embeddings()?;

    for request in request_rx {
        let MLRequest::Score { text, response_tx } = request;
        let topic = classify_topic(&classifier, &text);
        let quality = assess_quality(&classifier, &text);
        let (semantic_score, best_ref_idx) =
            semantic_similarity(&embeddings, &reference_embeddings, &text);

        let negative_rejection = topic.is_negative_label && topic.best_label_score >= 0.85;

        let _ = response_tx.send(MLScores {
            classification_score: topic.score,
            semantic_score,
            best_label: topic.best_label.clone(),
            best_label_score: topic.best_label_score,
            best_reference_idx: best_ref_idx,
            all_labels: topic.all_labels.clone(),
            is_negative_label: topic.is_negative_label,
            negative_rejection,
            topic,
            quality,
        });
    }

    Ok(())
}

fn classify_topic(classifier: &ZeroShotClassificationModel, text: &str) -> TopicClassification {
    let all_labels = TopicLabel::all_labels();

    let result = classifier.predict_multilabel(
        [text],
        &all_labels,
        Some(Box::new(|label| format!("This post is about {}.", label))),
        128,
    );

    match result {
        Ok(predictions) => {
            if let Some(labels) = predictions.first() {
                let mut all_scores: Vec<(String, f32)> = labels
                    .iter()
                    .map(|l| (l.text.clone(), l.score as f32))
                    .collect();
                all_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                if let Some((best_label, best_score)) = all_scores.first() {
                    let is_positive = TopicLabel::from_str(best_label)
                        .map(|l| l.is_positive())
                        .unwrap_or(false);
                    let is_negative = !is_positive;

                    let score = if is_positive {
                        *best_score
                    } else {
                        1.0 - best_score
                    };

                    TopicClassification {
                        score,
                        best_label: best_label.clone(),
                        best_label_score: *best_score,
                        all_labels: all_scores,
                        is_negative_label: is_negative,
                    }
                } else {
                    TopicClassification::default()
                }
            } else {
                TopicClassification::default()
            }
        }
        Err(_) => TopicClassification::default(),
    }
}

fn assess_quality(classifier: &ZeroShotClassificationModel, text: &str) -> QualityAssessment {
    let all_labels = QualityLabel::all_labels();

    let result = classifier.predict_multilabel(
        [text],
        &all_labels,
        Some(Box::new(|label| format!("This tweet sounds {}.", label))),
        128,
    );

    match result {
        Ok(predictions) => {
            if let Some(labels) = predictions.first() {
                let scores: std::collections::HashMap<String, f32> = labels
                    .iter()
                    .map(|l| (l.text.clone(), l.score as f32))
                    .collect();

                let engagement_bait_score = scores
                    .get(QualityLabel::EngagementBait.to_string().as_str())
                    .copied()
                    .unwrap_or(0.0);
                let synthetic_score = scores
                    .get(QualityLabel::Synthetic.to_string().as_str())
                    .copied()
                    .unwrap_or(0.0);

                QualityAssessment {
                    engagement_bait_score,
                    synthetic_score,
                }
            } else {
                QualityAssessment::default()
            }
        }
        Err(_) => QualityAssessment::default(),
    }
}

pub use TopicLabel as ClassificationLabel;
