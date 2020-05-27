table! {
    packages (id) {
        id -> Integer,
        name -> Text,
        version -> Text,
        status -> Text,
        distro -> Text,
        suite -> Text,
        architecture -> Text,
        url -> Text,
        built_at -> Nullable<Timestamp>,
        attestation -> Nullable<Text>,
        checksum -> Nullable<Text>,
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

joinable!(queue -> packages (package_id));
joinable!(queue -> workers (worker_id));

allow_tables_to_appear_in_same_query!(
    packages,
    queue,
    workers,
);
