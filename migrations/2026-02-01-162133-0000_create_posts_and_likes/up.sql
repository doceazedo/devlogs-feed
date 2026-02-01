CREATE TABLE posts (
    uri TEXT PRIMARY KEY NOT NULL,
    text TEXT NOT NULL,
    timestamp BIGINT NOT NULL,
    -- Scoring data
    final_score REAL NOT NULL,
    priority REAL NOT NULL,
    confidence TEXT NOT NULL,
    post_type TEXT NOT NULL,
    -- Signal breakdown (for debugging)
    keyword_score REAL NOT NULL,
    hashtag_score REAL NOT NULL,
    semantic_score REAL NOT NULL,
    classification_score REAL NOT NULL,
    has_media INTEGER NOT NULL,
    is_first_person INTEGER NOT NULL
);

CREATE TABLE likes (
    post_uri TEXT NOT NULL,
    like_uri TEXT NOT NULL,
    PRIMARY KEY (post_uri, like_uri),
    FOREIGN KEY (post_uri) REFERENCES posts(uri) ON DELETE CASCADE
);

-- Indexes for efficient queries
CREATE INDEX idx_posts_timestamp ON posts(timestamp DESC);
CREATE INDEX idx_posts_priority ON posts(priority DESC);
CREATE INDEX idx_likes_post_uri ON likes(post_uri);
