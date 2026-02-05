use crate::schema::{blocked_authors, likes, posts, user_interactions};
use crate::scoring::{ContentSignals, MediaInfo};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub fn establish_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .max_size(5)
        .build(manager)
        .expect("Failed to create pool")
}

pub fn configure_connection(conn: &mut SqliteConnection) -> QueryResult<()> {
    conn.batch_execute("PRAGMA busy_timeout = 2000;")?;
    conn.batch_execute("PRAGMA journal_mode = WAL;")?;
    conn.batch_execute("PRAGMA synchronous = NORMAL;")?;
    conn.batch_execute("PRAGMA foreign_keys = ON;")?;
    Ok(())
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = posts)]
#[allow(dead_code)]
pub struct Post {
    pub uri: String,
    pub text: String,
    pub timestamp: i64,
    pub priority: f32,
    pub has_media: i32,
    pub is_first_person: i32,
    pub author_did: Option<String>,
    pub image_count: i32,
    pub has_alt_text: i32,
    pub link_count: i32,
    pub promo_link_count: i32,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = posts)]
pub struct NewPost {
    pub uri: String,
    pub text: String,
    pub timestamp: i64,
    pub priority: f32,
    pub has_media: i32,
    pub is_first_person: i32,
    pub author_did: Option<String>,
    pub image_count: i32,
    pub has_alt_text: i32,
    pub link_count: i32,
    pub promo_link_count: i32,
}

impl NewPost {
    pub fn new(
        uri: String,
        text: String,
        timestamp: i64,
        priority: f32,
        media: &MediaInfo,
        content: &ContentSignals,
        author_did: Option<String>,
    ) -> Self {
        Self {
            uri,
            text,
            timestamp,
            priority,
            has_media: if media.image_count > 0 || media.has_video {
                1
            } else {
                0
            },
            is_first_person: i32::from(content.is_first_person),
            author_did,
            image_count: content.images as i32,
            has_alt_text: i32::from(content.has_alt_text),
            link_count: content.link_count as i32,
            promo_link_count: content.promo_link_count as i32,
        }
    }
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = likes)]
pub struct NewLike {
    pub post_uri: String,
    pub like_uri: String,
}

pub fn insert_posts(conn: &mut SqliteConnection, new_posts: Vec<NewPost>) -> QueryResult<usize> {
    use crate::schema::posts::dsl::*;

    diesel::insert_or_ignore_into(posts)
        .values(&new_posts)
        .execute(conn)
}

pub fn insert_likes(conn: &mut SqliteConnection, new_likes: Vec<NewLike>) -> QueryResult<usize> {
    use crate::schema::likes::dsl::*;
    use crate::schema::posts::dsl::posts;
    use crate::schema::posts::dsl::uri as post_uri_col;

    if new_likes.is_empty() {
        return Ok(0);
    }

    let post_uris: Vec<&str> = new_likes.iter().map(|l| l.post_uri.as_str()).collect();

    let existing_posts: Vec<String> = posts
        .filter(post_uri_col.eq_any(&post_uris))
        .select(post_uri_col)
        .load(conn)?;

    let valid_likes: Vec<_> = new_likes
        .into_iter()
        .filter(|l| existing_posts.contains(&l.post_uri))
        .collect();

    if valid_likes.is_empty() {
        return Ok(0);
    }

    diesel::insert_or_ignore_into(likes)
        .values(&valid_likes)
        .execute(conn)
}

pub fn delete_post(conn: &mut SqliteConnection, post_uri: &str) -> QueryResult<usize> {
    use crate::schema::posts::dsl::*;

    diesel::delete(posts.filter(uri.eq(post_uri))).execute(conn)
}

pub fn delete_like(conn: &mut SqliteConnection, like_uri_val: &str) -> QueryResult<usize> {
    use crate::schema::likes::dsl::*;

    diesel::delete(likes.filter(like_uri.eq(like_uri_val))).execute(conn)
}

pub fn get_feed(conn: &mut SqliteConnection, cutoff_timestamp: i64) -> QueryResult<Vec<Post>> {
    use crate::schema::posts::dsl::*;

    posts
        .filter(timestamp.gt(cutoff_timestamp))
        .order((timestamp.desc(), priority.desc()))
        .load::<Post>(conn)
}

pub fn post_exists(conn: &mut SqliteConnection, post_uri: &str) -> bool {
    use crate::schema::posts::dsl::*;

    posts
        .filter(uri.eq(post_uri))
        .count()
        .get_result::<i64>(conn)
        .unwrap_or(0)
        > 0
}

pub fn get_post_author(conn: &mut SqliteConnection, post_uri: &str) -> Option<String> {
    use crate::schema::posts::dsl::*;

    posts
        .filter(uri.eq(post_uri))
        .select(author_did)
        .first::<Option<String>>(conn)
        .ok()
        .flatten()
}

pub fn cleanup_old_posts(
    conn: &mut SqliteConnection,
    cutoff_timestamp: i64,
    max_posts: i64,
) -> QueryResult<usize> {
    use crate::schema::posts::dsl::*;

    let deleted_by_age =
        diesel::delete(posts.filter(timestamp.lt(cutoff_timestamp))).execute(conn)?;

    let count: i64 = posts.count().get_result(conn)?;
    let mut deleted_by_limit = 0;
    if count > max_posts {
        let excess = count - max_posts;
        let uris_to_delete: Vec<String> = posts
            .order(timestamp.asc())
            .limit(excess)
            .select(uri)
            .load(conn)?;

        deleted_by_limit =
            diesel::delete(posts.filter(uri.eq_any(uris_to_delete))).execute(conn)?;
    }

    Ok(deleted_by_age + deleted_by_limit)
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = user_interactions)]
pub struct NewInteraction {
    pub user_did: String,
    pub post_uri: String,
    pub interaction_type: String,
    pub created_at: i64,
}

pub const INTERACTION_SEEN: &str = "seen";
pub const INTERACTION_REQUEST_LESS: &str = "request_less";
pub const INTERACTION_REQUEST_MORE: &str = "request_more";

pub fn insert_interactions(
    conn: &mut SqliteConnection,
    interactions: Vec<NewInteraction>,
) -> QueryResult<usize> {
    use crate::schema::user_interactions::dsl::*;

    if interactions.is_empty() {
        return Ok(0);
    }

    diesel::insert_or_ignore_into(user_interactions)
        .values(&interactions)
        .execute(conn)
}

pub fn get_user_seen_posts(
    conn: &mut SqliteConnection,
    did: &str,
    cutoff_timestamp: i64,
) -> QueryResult<Vec<String>> {
    use crate::schema::user_interactions::dsl::*;

    user_interactions
        .filter(user_did.eq(did))
        .filter(interaction_type.eq(INTERACTION_SEEN))
        .filter(created_at.gt(cutoff_timestamp))
        .select(post_uri)
        .load(conn)
}

#[derive(Debug, Clone)]
pub struct UserPreference {
    pub post_uri: String,
    pub is_request_more: bool,
}

pub fn get_user_preferences(
    conn: &mut SqliteConnection,
    did: &str,
) -> QueryResult<Vec<UserPreference>> {
    use crate::schema::user_interactions::dsl::*;

    let results: Vec<(String, String)> = user_interactions
        .filter(user_did.eq(did))
        .filter(
            interaction_type
                .eq(INTERACTION_REQUEST_LESS)
                .or(interaction_type.eq(INTERACTION_REQUEST_MORE)),
        )
        .select((post_uri, interaction_type))
        .load(conn)?;

    Ok(results
        .into_iter()
        .map(|(uri, itype)| UserPreference {
            post_uri: uri,
            is_request_more: itype == INTERACTION_REQUEST_MORE,
        })
        .collect())
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = blocked_authors)]
pub struct NewBlockedAuthor {
    pub did: String,
    pub post_uri: String,
    pub blocked_at: i64,
}

pub fn is_blocked_author(conn: &mut SqliteConnection, author_did: &str) -> bool {
    blocked_authors::table
        .filter(blocked_authors::did.eq(author_did))
        .count()
        .get_result::<i64>(conn)
        .unwrap_or(0)
        > 0
}

pub fn block_author(conn: &mut SqliteConnection, blocked: NewBlockedAuthor) -> QueryResult<usize> {
    diesel::insert_or_ignore_into(blocked_authors::table)
        .values(&blocked)
        .execute(conn)
}

pub fn delete_posts_by_author(conn: &mut SqliteConnection, did: &str) -> QueryResult<usize> {
    use crate::schema::posts::dsl::*;

    diesel::delete(posts.filter(author_did.eq(did))).execute(conn)
}
