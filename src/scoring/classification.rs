use anyhow::Result;
use rust_bert::pipelines::zero_shot_classification::ZeroShotClassificationModel;
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use strum::{Display, EnumIter, IntoEnumIterator, IntoStaticStr};

use super::semantic::{compute_reference_embeddings, semantic_similarity};
use crate::utils::{log_ml_error, log_ml_model_loaded, log_ml_ready, log_ml_shutdown, log_ml_step};

pub const WEIGHT_CLASSIFICATION: f32 = 0.50;
pub const NEGATIVE_REJECTION_THRESHOLD: f32 = 0.70;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumIter, IntoStaticStr)]
pub enum ClassificationLabel {
    // good :)
    #[strum(to_string = "game developer sharing their own work")]
    GameDevSharingWork,
    #[strum(to_string = "game programming or technical development")]
    GameProgramming,

    // bad :(
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
    CryptoAndNFTs,
    #[strum(to_string = "unrelated")]
    Unrelated,
}

impl ClassificationLabel {
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
            Self::CryptoAndNFTs => 0.0,
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

impl FromStr for ClassificationLabel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::iter().find(|l| l.to_string() == s).ok_or(())
    }
}

pub fn label_multiplier(label: &str) -> f32 {
    ClassificationLabel::from_str(label)
        .map(|l| l.multiplier())
        .unwrap_or(0.6)
}

pub enum MLRequest {
    Score {
        text: String,
        response_tx: tokio::sync::oneshot::Sender<MLScores>,
    },
    Shutdown,
}

#[derive(Debug, Clone, Default)]
pub struct MLScores {
    pub classification_score: f32,
    pub semantic_score: f32,
    pub best_label: String,
    pub best_label_score: f32,
    pub best_reference_idx: usize,
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
            if let Err(e) = run_ml_worker(request_rx) {
                log_ml_error(&format!("Worker failed: {e}"));
            }
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
            log_ml_error("Worker channel closed");
            return MLScores::default();
        }

        response_rx.await.unwrap_or_default()
    }
}

fn run_ml_worker(request_rx: mpsc::Receiver<MLRequest>) -> Result<()> {
    log_ml_step("Loading zero-shot classification model...");
    let start = Instant::now();
    let classifier = ZeroShotClassificationModel::new(Default::default())?;
    log_ml_model_loaded("Zero-shot model", start.elapsed().as_secs_f32());

    let (embeddings, reference_embeddings) = compute_reference_embeddings()?;

    log_ml_ready();

    for request in request_rx {
        match request {
            MLRequest::Score { text, response_tx } => {
                let classification = classify(&classifier, &text);
                let (semantic_score, best_ref_idx) =
                    semantic_similarity(&embeddings, &reference_embeddings, &text);

                let _ = response_tx.send(MLScores {
                    classification_score: classification.score,
                    semantic_score,
                    best_label: classification.best_label,
                    best_label_score: classification.best_label_score,
                    best_reference_idx: best_ref_idx,
                    all_labels: classification.all_labels,
                    is_negative_label: classification.is_negative_label,
                    negative_rejection: classification.negative_rejection,
                });
            }
            MLRequest::Shutdown => {
                log_ml_shutdown();
                break;
            }
        }
    }

    Ok(())
}

pub struct ClassificationResult {
    pub score: f32,
    pub best_label: String,
    pub best_label_score: f32,
    pub all_labels: Vec<(String, f32)>,
    pub is_negative_label: bool,
    pub negative_rejection: bool,
}

impl Default for ClassificationResult {
    fn default() -> Self {
        Self {
            score: 0.0,
            best_label: String::new(),
            best_label_score: 0.0,
            all_labels: Vec::new(),
            is_negative_label: false,
            negative_rejection: false,
        }
    }
}

fn classify(classifier: &ZeroShotClassificationModel, text: &str) -> ClassificationResult {
    let all_labels = ClassificationLabel::all_labels();

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
                    let is_positive = ClassificationLabel::from_str(best_label)
                        .map(|l| l.is_positive())
                        .unwrap_or(false);
                    let is_negative = !is_positive;

                    let score = if is_positive {
                        *best_score
                    } else {
                        1.0 - best_score
                    };

                    let negative_rejection =
                        is_negative && *best_score >= NEGATIVE_REJECTION_THRESHOLD;

                    ClassificationResult {
                        score,
                        best_label: best_label.clone(),
                        best_label_score: *best_score,
                        all_labels: all_scores,
                        is_negative_label: is_negative,
                        negative_rejection,
                    }
                } else {
                    ClassificationResult::default()
                }
            } else {
                ClassificationResult::default()
            }
        }
        Err(_) => ClassificationResult::default(),
    }
}
