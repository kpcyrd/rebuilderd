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
    }
}

table! {
    queue (id) {
        id -> Integer,
        package_id -> Integer,
        version -> Text,
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
