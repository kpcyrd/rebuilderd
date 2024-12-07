// @generated automatically by Diesel CLI.

diesel::table! {
    builds (id) {
        id -> Integer,
        diffoscope -> Nullable<Text>,
        build_log -> Binary,
        attestation -> Nullable<Text>,
    }
}

diesel::table! {
    packages (id) {
        id -> Integer,
        pkgbase_id -> Integer,
        name -> Text,
        version -> Text,
        status -> Text,
        distro -> Text,
        suite -> Text,
        architecture -> Text,
        artifact_url -> Text,
        build_id -> Nullable<Integer>,
        built_at -> Nullable<Timestamp>,
        has_diffoscope -> Bool,
        has_attestation -> Bool,
        checksum -> Nullable<Text>,
    }
}

diesel::table! {
    pkgbases (id) {
        id -> Integer,
        name -> Text,
        version -> Text,
        distro -> Text,
        suite -> Text,
        architecture -> Text,
        input_url -> Nullable<Text>,
        artifacts -> Text,
        retries -> Integer,
        next_retry -> Nullable<Timestamp>,
    }
}

diesel::table! {
    queue (id) {
        id -> Integer,
        pkgbase_id -> Integer,
        version -> Text,
        required_backend -> Text,
        priority -> Integer,
        queued_at -> Timestamp,
        worker_id -> Nullable<Integer>,
        started_at -> Nullable<Timestamp>,
        last_ping -> Nullable<Timestamp>,
    }
}

diesel::table! {
    workers (id) {
        id -> Integer,
        key -> Text,
        addr -> Text,
        status -> Nullable<Text>,
        last_ping -> Timestamp,
        online -> Bool,
    }
}

diesel::joinable!(packages -> builds (build_id));
diesel::joinable!(packages -> pkgbases (pkgbase_id));
diesel::joinable!(queue -> pkgbases (pkgbase_id));
diesel::joinable!(queue -> workers (worker_id));

diesel::allow_tables_to_appear_in_same_query!(
    builds,
    packages,
    pkgbases,
    queue,
    workers,
);
