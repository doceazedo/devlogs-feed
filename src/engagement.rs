use crate::db::DbPool;
use crate::schema::{engagement_cache, replies, reposts, spammers};
use crate::settings::settings;
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

#[derive(Debug)]
#[allow(dead_code)]
pub struct SpamDetected {
    pub did: String,
    pub reason: String,
    pub frequency: Option<f32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = reposts)]
pub struct NewRepost {
    pub post_uri: String,
    pub repost_uri: String,
    pub reposter_did: String,
    pub timestamp: i64,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = replies)]
pub struct NewReply {
    pub post_uri: String,
    pub reply_uri: String,
    pub author_did: String,
    pub timestamp: i64,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = spammers)]
pub struct NewSpammer {
    pub did: String,
    pub reason: String,
    pub repost_frequency: Option<f32>,
    pub flagged_at: i64,
    pub auto_detected: i32,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = spammers)]
#[allow(dead_code)]
pub struct Spammer {
    pub did: String,
    pub reason: String,
    pub repost_frequency: Option<f32>,
    pub flagged_at: i64,
    pub auto_detected: i32,
}

#[derive(Insertable, AsChangeset, Debug)]
#[diesel(table_name = engagement_cache)]
pub struct EngagementCacheEntry {
    pub post_uri: String,
    pub reply_count: i32,
    pub repost_count: i32,
    pub like_count: i32,
    pub velocity_score: f32,
    pub last_updated: i64,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = engagement_cache)]
#[allow(dead_code)]
pub struct EngagementCache {
    pub post_uri: String,
    pub reply_count: i32,
    pub repost_count: i32,
    pub like_count: i32,
    pub velocity_score: f32,
    pub last_updated: i64,
}

#[derive(Clone)]
pub struct EngagementTracker {
    pool: DbPool,
}

impl EngagementTracker {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    pub fn record_repost(
        &self,
        post_uri: &str,
        repost_uri: &str,
        reposter_did: &str,
    ) -> Result<(), SpamDetected> {
        let mut conn = self.pool.get().map_err(|_| SpamDetected {
            did: reposter_did.to_string(),
            reason: "database error".to_string(),
            frequency: None,
        })?;

        if let Some(spam) = self.check_repost_spam(&mut conn, reposter_did) {
            self.flag_spammer_internal(&mut conn, reposter_did, &spam.reason, spam.frequency)
                .ok();
            return Err(spam);
        }

        let new_repost = NewRepost {
            post_uri: post_uri.to_string(),
            repost_uri: repost_uri.to_string(),
            reposter_did: reposter_did.to_string(),
            timestamp: Utc::now().timestamp(),
        };

        diesel::insert_or_ignore_into(reposts::table)
            .values(&new_repost)
            .execute(&mut conn)
            .ok();

        self.update_engagement_cache(&mut conn, post_uri).ok();

        Ok(())
    }

    pub fn record_like(&self, post_uri: &str) -> Result<(), DieselError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|_| DieselError::BrokenTransactionManager)?;
        self.update_engagement_cache(&mut conn, post_uri)
    }

    fn check_repost_spam(
        &self,
        conn: &mut diesel::SqliteConnection,
        reposter_did: &str,
    ) -> Option<SpamDetected> {
        let s = settings();
        let now = Utc::now().timestamp();
        let window_start = now - (s.spam.velocity_window_hours * 3600);

        let recent_count: i64 = reposts::table
            .filter(reposts::reposter_did.eq(reposter_did))
            .filter(reposts::timestamp.gt(window_start))
            .count()
            .get_result(conn)
            .unwrap_or(0);

        let frequency = recent_count as f32 / s.spam.velocity_window_hours as f32;

        if frequency >= s.spam.repost_threshold {
            return Some(SpamDetected {
                did: reposter_did.to_string(),
                reason: format!("high repost frequency: {:.1}/hr", frequency),
                frequency: Some(frequency),
            });
        }

        None
    }

    fn update_engagement_cache(
        &self,
        conn: &mut diesel::SqliteConnection,
        post_uri: &str,
    ) -> Result<(), DieselError> {
        let s = settings();
        let now = Utc::now().timestamp();
        let window_start = now - (s.spam.velocity_window_hours * 3600);

        let reply_count: i64 = replies::table
            .filter(replies::post_uri.eq(post_uri))
            .count()
            .get_result(conn)
            .unwrap_or(0);

        let recent_replies: i64 = replies::table
            .filter(replies::post_uri.eq(post_uri))
            .filter(replies::timestamp.gt(window_start))
            .count()
            .get_result(conn)
            .unwrap_or(0);

        let repost_count: i64 = reposts::table
            .filter(reposts::post_uri.eq(post_uri))
            .count()
            .get_result(conn)
            .unwrap_or(0);

        let recent_reposts: i64 = reposts::table
            .filter(reposts::post_uri.eq(post_uri))
            .filter(reposts::timestamp.gt(window_start))
            .count()
            .get_result(conn)
            .unwrap_or(0);

        let like_count: i64 = crate::schema::likes::table
            .filter(crate::schema::likes::post_uri.eq(post_uri))
            .count()
            .get_result(conn)
            .unwrap_or(0);

        let velocity = recent_replies as f32 * s.engagement.weights.reply
            + recent_reposts as f32 * s.engagement.weights.repost
            + like_count as f32 * s.engagement.weights.like * 0.1;

        let entry = EngagementCacheEntry {
            post_uri: post_uri.to_string(),
            reply_count: reply_count as i32,
            repost_count: repost_count as i32,
            like_count: like_count as i32,
            velocity_score: velocity,
            last_updated: now,
        };

        diesel::insert_into(engagement_cache::table)
            .values(&entry)
            .on_conflict(engagement_cache::post_uri)
            .do_update()
            .set(&entry)
            .execute(conn)?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_engagement(&self, post_uri: &str) -> Option<EngagementCache> {
        let mut conn = self.pool.get().ok()?;

        engagement_cache::table
            .filter(engagement_cache::post_uri.eq(post_uri))
            .first(&mut conn)
            .ok()
    }

    pub fn is_spammer(&self, did: &str) -> bool {
        let mut conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return false,
        };

        self.is_spammer_internal(&mut conn, did)
    }

    fn is_spammer_internal(&self, conn: &mut diesel::SqliteConnection, did: &str) -> bool {
        spammers::table
            .filter(spammers::did.eq(did))
            .count()
            .get_result::<i64>(conn)
            .unwrap_or(0)
            > 0
    }

    #[allow(dead_code)]
    pub fn flag_spammer(&self, did: &str, reason: &str) -> Result<(), DieselError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|_| DieselError::BrokenTransactionManager)?;
        self.flag_spammer_internal(&mut conn, did, reason, None)
    }

    fn flag_spammer_internal(
        &self,
        conn: &mut diesel::SqliteConnection,
        did: &str,
        reason: &str,
        frequency: Option<f32>,
    ) -> Result<(), DieselError> {
        let new_spammer = NewSpammer {
            did: did.to_string(),
            reason: reason.to_string(),
            repost_frequency: frequency,
            flagged_at: Utc::now().timestamp(),
            auto_detected: if frequency.is_some() { 1 } else { 0 },
        };

        diesel::insert_or_ignore_into(spammers::table)
            .values(&new_spammer)
            .execute(conn)?;

        Ok(())
    }

    pub fn cleanup_old_engagement(&self, cutoff_timestamp: i64) -> Result<usize, DieselError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|_| DieselError::BrokenTransactionManager)?;

        let deleted_replies =
            diesel::delete(replies::table.filter(replies::timestamp.lt(cutoff_timestamp)))
                .execute(&mut conn)?;

        let deleted_reposts =
            diesel::delete(reposts::table.filter(reposts::timestamp.lt(cutoff_timestamp)))
                .execute(&mut conn)?;

        Ok(deleted_replies + deleted_reposts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_velocity_calculation() {
        let s = settings();
        let velocity = 5.0 * s.engagement.weights.reply
            + 3.0 * s.engagement.weights.repost
            + 10.0 * s.engagement.weights.like * 0.1;
        assert!((velocity - 22.0).abs() < 0.01);
    }
}
