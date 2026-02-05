use anyhow::Result;
use rust_bert::pipelines::zero_shot_classification::ZeroShotClassificationModel;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use strum::{Display, EnumIter, IntoEnumIterator, IntoStaticStr};

use crate::settings::settings;

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
pub struct QualityAssessment {
    pub engagement_bait_score: f32,
    pub synthetic_score: f32,
    pub authenticity_score: f32,
}

pub enum MLRequest {
    Score {
        text: String,
        response_tx: tokio::sync::oneshot::Sender<QualityAssessment>,
    },
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

    pub async fn score(&self, text: String) -> QualityAssessment {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        if self
            .request_tx
            .send(MLRequest::Score { text, response_tx })
            .is_err()
        {
            return QualityAssessment::default();
        }

        response_rx.await.unwrap_or_default()
    }
}

fn run_ml_worker(request_rx: mpsc::Receiver<MLRequest>) -> Result<()> {
    let classifier = ZeroShotClassificationModel::new(Default::default())?;
    let s = settings();
    let batch_timeout = Duration::from_millis(s.ml.batch_timeout_ms);

    loop {
        let mut batch: Vec<(String, tokio::sync::oneshot::Sender<QualityAssessment>)> = Vec::new();

        match request_rx.recv() {
            Ok(MLRequest::Score { text, response_tx }) => {
                batch.push((text, response_tx));
            }
            Err(_) => break,
        }

        while batch.len() < s.ml.batch_size {
            match request_rx.recv_timeout(batch_timeout) {
                Ok(MLRequest::Score { text, response_tx }) => {
                    batch.push((text, response_tx));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        if batch.is_empty() {
            continue;
        }

        let texts: Vec<&str> = batch.iter().map(|(t, _)| t.as_str()).collect();
        let qualities = assess_quality_batch(&classifier, &texts);

        for (i, (_, response_tx)) in batch.into_iter().enumerate() {
            let quality = qualities.get(i).cloned().unwrap_or_default();
            let _ = response_tx.send(quality);
        }
    }

    Ok(())
}

fn assess_quality_batch(
    classifier: &ZeroShotClassificationModel,
    texts: &[&str],
) -> Vec<QualityAssessment> {
    let all_labels = QualityLabel::all_labels();

    let result = classifier.predict_multilabel(
        texts,
        &all_labels,
        Some(Box::new(|label| format!("This tweet sounds {}.", label))),
        128,
    );

    match result {
        Ok(predictions) => predictions
            .iter()
            .map(|labels| {
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
                let authenticity_score = scores
                    .get(QualityLabel::Authentic.to_string().as_str())
                    .copied()
                    .unwrap_or(0.0);

                QualityAssessment {
                    engagement_bait_score,
                    synthetic_score,
                    authenticity_score,
                }
            })
            .collect(),
        Err(_) => vec![QualityAssessment::default(); texts.len()],
    }
}
