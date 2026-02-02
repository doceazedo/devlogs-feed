// @generated automatically by Diesel CLI.

diesel::table! {
    engagement_cache (post_uri) {
        post_uri -> Text,
        reply_count -> Integer,
        repost_count -> Integer,
        like_count -> Integer,
        velocity_score -> Float,
        last_updated -> BigInt,
    }
}

diesel::table! {
    likes (post_uri, like_uri) {
        post_uri -> Text,
        like_uri -> Text,
    }
}

diesel::table! {
    posts (uri) {
        uri -> Text,
        text -> Text,
        timestamp -> BigInt,
        final_score -> Float,
        priority -> Float,
        confidence -> Text,
        post_type -> Text,
        keyword_score -> Float,
        hashtag_score -> Float,
        semantic_score -> Float,
        classification_score -> Float,
        has_media -> Integer,
        is_first_person -> Integer,
        author_did -> Nullable<Text>,
        image_count -> Integer,
        has_alt_text -> Integer,
        link_count -> Integer,
        promo_link_count -> Integer,
    }
}

diesel::table! {
    replies (post_uri, reply_uri) {
        post_uri -> Text,
        reply_uri -> Text,
        author_did -> Text,
        timestamp -> BigInt,
    }
}

diesel::table! {
    reposts (post_uri, repost_uri) {
        post_uri -> Text,
        repost_uri -> Text,
        reposter_did -> Text,
        timestamp -> BigInt,
    }
}

diesel::table! {
    spammers (did) {
        did -> Text,
        reason -> Text,
        repost_frequency -> Nullable<Float>,
        flagged_at -> BigInt,
        auto_detected -> Integer,
    }
}

diesel::joinable!(engagement_cache -> posts (post_uri));
diesel::joinable!(likes -> posts (post_uri));
diesel::joinable!(replies -> posts (post_uri));
diesel::joinable!(reposts -> posts (post_uri));

diesel::allow_tables_to_appear_in_same_query!(
    engagement_cache,
    likes,
    posts,
    replies,
    reposts,
    spammers,
);
