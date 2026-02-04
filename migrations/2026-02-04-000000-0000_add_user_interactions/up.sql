CREATE TABLE user_interactions (
    user_did TEXT NOT NULL,
    post_uri TEXT NOT NULL,
    interaction_type TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (user_did, post_uri, interaction_type)
);

CREATE INDEX idx_interactions_user_did ON user_interactions(user_did);
CREATE INDEX idx_interactions_user_type ON user_interactions(user_did, interaction_type);
CREATE INDEX idx_interactions_created_at ON user_interactions(created_at);
