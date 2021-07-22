table! {
    builds (id) {
        id -> Integer,
        diffoscope -> Nullable<Text>,
        build_log -> Binary,
    }
}

table! {
    packages (id) {
        id -> Integer,
        base_id -> Nullable<Integer>,
        name -> Text,
        version -> Text,
        status -> Text,
        distro -> Text,
        suite -> Text,
        architecture -> Text,
        url -> Text,
        build_id -> Nullable<Integer>,
        built_at -> Nullable<Timestamp>,
        has_diffoscope -> Bool,
        attestation -> Nullable<Text>,
        checksum -> Nullable<Text>,
        retries -> Integer,
        next_retry -> Nullable<Timestamp>,
    }
}

table! {
    pkgbases (id) {
        id -> Integer,
        name -> Text,
        version -> Text,
        distro -> Text,
        suite -> Text,
        architecture -> Text,
        retries -> Integer,
        next_retry -> Nullable<Timestamp>,
    }
}

table! {
    queue (id) {
        id -> Integer,
        package_id -> Integer,
        version -> Text,
        priority -> Integer,
        queued_at -> Timestamp,
        worker_id -> Nullable<Integer>,
        started_at -> Nullable<Timestamp>,
        last_ping -> Nullable<Timestamp>,
    }
}

table! {
    workers (id) {
        id -> Integer,
        key -> Text,
        addr -> Text,
        status -> Nullable<Text>,
        last_ping -> Timestamp,
        online -> Bool,
    }
}

joinable!(packages -> builds (build_id));
joinable!(packages -> pkgbases (base_id));
joinable!(queue -> packages (package_id));
joinable!(queue -> workers (worker_id));

allow_tables_to_appear_in_same_query!(
    builds,
    packages,
    pkgbases,
    queue,
    workers,
);
