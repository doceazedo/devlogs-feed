use crate::schema::{likes, posts};
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
    pub final_score: f32,
    pub priority: f32,
    pub confidence: String,
    pub post_type: String,
    pub keyword_score: f32,
    pub hashtag_score: f32,
    pub semantic_score: f32,
    pub classification_score: f32,
    pub has_media: i32,
    pub is_first_person: i32,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = posts)]
pub struct NewPost {
    pub uri: String,
    pub text: String,
    pub timestamp: i64,
    pub final_score: f32,
    pub priority: f32,
    pub confidence: String,
    pub post_type: String,
    pub keyword_score: f32,
    pub hashtag_score: f32,
    pub semantic_score: f32,
    pub classification_score: f32,
    pub has_media: i32,
    pub is_first_person: i32,
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
        .order(final_score.desc())
        .load::<Post>(conn)
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

pub fn get_latest_post_timestamp(conn: &mut SqliteConnection) -> QueryResult<Option<i64>> {
    use crate::schema::posts::dsl::*;

    posts
        .select(diesel::dsl::max(timestamp))
        .first::<Option<i64>>(conn)
}
