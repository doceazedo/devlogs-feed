// @generated automatically by Diesel CLI.

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
    }
}

diesel::joinable!(likes -> posts (post_uri));

diesel::allow_tables_to_appear_in_same_query!(likes, posts,);
