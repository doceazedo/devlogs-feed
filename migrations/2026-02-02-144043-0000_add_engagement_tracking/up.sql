ALTER TABLE posts ADD COLUMN author_did TEXT;
ALTER TABLE posts ADD COLUMN image_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE posts ADD COLUMN has_alt_text INTEGER NOT NULL DEFAULT 0;
ALTER TABLE posts ADD COLUMN link_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE posts ADD COLUMN promo_link_count INTEGER NOT NULL DEFAULT 0;

CREATE TABLE reposts (
    post_uri TEXT NOT NULL,
    repost_uri TEXT NOT NULL,
    reposter_did TEXT NOT NULL,
    timestamp BIGINT NOT NULL,
    PRIMARY KEY (post_uri, repost_uri),
    FOREIGN KEY (post_uri) REFERENCES posts(uri) ON DELETE CASCADE
);

CREATE TABLE replies (
    post_uri TEXT NOT NULL,
    reply_uri TEXT NOT NULL,
    author_did TEXT NOT NULL,
    timestamp BIGINT NOT NULL,
    PRIMARY KEY (post_uri, reply_uri),
    FOREIGN KEY (post_uri) REFERENCES posts(uri) ON DELETE CASCADE
);

CREATE TABLE spammers (
    did TEXT PRIMARY KEY NOT NULL,
    reason TEXT NOT NULL,
    repost_frequency REAL,
    flagged_at BIGINT NOT NULL,
    auto_detected INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE engagement_cache (
    post_uri TEXT PRIMARY KEY NOT NULL,
    reply_count INTEGER NOT NULL DEFAULT 0,
    repost_count INTEGER NOT NULL DEFAULT 0,
    like_count INTEGER NOT NULL DEFAULT 0,
    velocity_score REAL NOT NULL DEFAULT 0.0,
    last_updated BIGINT NOT NULL,
    FOREIGN KEY (post_uri) REFERENCES posts(uri) ON DELETE CASCADE
);

CREATE INDEX idx_reposts_reposter_did ON reposts(reposter_did);
CREATE INDEX idx_replies_post_uri ON replies(post_uri);
CREATE INDEX idx_engagement_velocity ON engagement_cache(velocity_score DESC);
CREATE INDEX idx_posts_author_did ON posts(author_did);
