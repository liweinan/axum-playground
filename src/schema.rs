// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Varchar,
        username -> Varchar,
        created_at -> Nullable<Timestamp>,
        meta -> Nullable<Jsonb>,
    }
}
