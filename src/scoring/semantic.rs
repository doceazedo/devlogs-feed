use anyhow::Result;
use rust_bert::pipelines::sentence_embeddings::{
    SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType,
};
use simsimd::SpatialSimilarity;

pub const REFERENCE_POSTS: &[&str] = &[
    "Just implemented the new combat system, feels so satisfying!",
    "Week 12 devlog: added procedural terrain generation",
    "Finally got the shader working, here's how it looks",
    "Spent the weekend debugging physics, but it's working now",
    "New character animations are in, movement feels much better",
    "Progress update: inventory system is almost complete",
    "Added save/load functionality to my game today",
    "Here's this week's indie dev progress on my roguelike",
    "Got the lighting system working after 3 days of debugging",
    "First playable build is ready for testing!",
    "Working on a particle system for my game engine",
    "Built a custom ECS framework for my project",
    "Finally got GPU instancing working, huge performance boost!",
    "Writing a tilemap renderer from scratch, learning so much",
    "My custom level editor is coming along nicely",
    "Some great progress on the SSGI implementation for the Godot Engine",
    "Making good progress on my Bevy game, loving the ECS",
    "Created a plugin for my game engine that handles audio",
    "Finished the pixel art for the main character",
    "Implemented A* pathfinding for enemy AI today",
    "Finally got multiplayer netcode syncing properly",
    "Redesigned the main menu UI, much cleaner now",
    "Added procedural dungeon generation to the game",
    "Spent the day profiling and fixed a major memory leak",
    "Working on the dialogue system, branching convos are tricky",
    "Just composed the boss battle theme, sounds epic",
];

pub fn compute_reference_embeddings() -> Result<(SentenceEmbeddingsModel, Vec<Vec<f32>>)> {
    let embeddings = SentenceEmbeddingsBuilder::remote(SentenceEmbeddingsModelType::AllMiniLmL12V2)
        .create_model()?;
    let reference_embeddings = embeddings.encode(REFERENCE_POSTS)?;
    Ok((embeddings, reference_embeddings))
}

pub fn semantic_similarity(
    embeddings: &SentenceEmbeddingsModel,
    reference_embeddings: &[Vec<f32>],
    text: &str,
) -> (f32, usize) {
    let result = embeddings.encode(&[text]);

    match result {
        Ok(text_embeddings) => {
            if let Some(text_embedding) = text_embeddings.first() {
                let mut best_idx = 0;
                let mut best_sim = 0.0_f32;

                for (idx, ref_emb) in reference_embeddings.iter().enumerate() {
                    let sim = cosine_similarity(text_embedding, ref_emb);
                    if sim > best_sim {
                        best_sim = sim;
                        best_idx = idx;
                    }
                }

                (best_sim, best_idx)
            } else {
                (0.0, 0)
            }
        }
        Err(_) => (0.0, 0),
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    f32::cosine(a, b)
        .map(|distance| (1.0 - distance) as f32)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_matching() {
        assert!(!REFERENCE_POSTS.is_empty());
        assert!(REFERENCE_POSTS.len() >= 20);
    }
}
