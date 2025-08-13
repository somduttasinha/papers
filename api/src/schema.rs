// @generated automatically by Diesel CLI.

diesel::table! {
    documents (id) {
        id -> Varchar,
        title -> Varchar,
        body -> Text,
        thumbnail_url -> Varchar,
    }
}
