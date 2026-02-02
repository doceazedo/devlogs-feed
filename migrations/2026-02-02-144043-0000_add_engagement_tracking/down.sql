DROP INDEX IF EXISTS idx_posts_author_did;
DROP INDEX IF EXISTS idx_engagement_velocity;
DROP INDEX IF EXISTS idx_replies_post_uri;
DROP INDEX IF EXISTS idx_reposts_reposter_did;

DROP TABLE IF EXISTS engagement_cache;
DROP TABLE IF EXISTS spammers;
DROP TABLE IF EXISTS replies;
DROP TABLE IF EXISTS reposts;

ALTER TABLE posts DROP COLUMN promo_link_count;
ALTER TABLE posts DROP COLUMN link_count;
ALTER TABLE posts DROP COLUMN has_alt_text;
ALTER TABLE posts DROP COLUMN image_count;
ALTER TABLE posts DROP COLUMN author_did;
