CREATE TABLE blocked_authors (
    did TEXT PRIMARY KEY NOT NULL,
    post_uri TEXT NOT NULL,
    blocked_at BIGINT NOT NULL
);
