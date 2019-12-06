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
    workers (id) {
        id -> Integer,
        key -> Text,
        addr -> Text,
        status -> Nullable<Text>,
        last_ping -> Timestamp,
        online -> Bool,
    }
}

allow_tables_to_appear_in_same_query!(
    packages,
    workers,
);
