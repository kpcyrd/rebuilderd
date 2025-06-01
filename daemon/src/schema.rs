// @generated automatically by Diesel CLI.

diesel::table! {
    binary_packages (id) {
        id -> Integer,
        source_package_id -> Integer,
        build_input_id -> Integer,
        name -> Text,
        version -> Text,
        architecture -> Text,
        artifact_url -> Text,
    }
}

diesel::table! {
    build_inputs (id) {
        id -> Integer,
        source_package_id -> Integer,
        url -> Text,
        backend -> Text,
        architecture -> Text,
        retries -> Integer,
        next_retry -> Nullable<Timestamp>,
    }
}

diesel::table! {
    queue (id) {
        id -> Integer,
        build_input_id -> Integer,
        priority -> Integer,
        queued_at -> Timestamp,
        started_at -> Nullable<Timestamp>,
        worker -> Nullable<Integer>,
        last_ping -> Nullable<Timestamp>,
    }
}

diesel::table! {
    rebuild_artifacts (id) {
        id -> Integer,
        rebuild_id -> Integer,
        name -> Text,
        diffoscope -> Nullable<Binary>,
        attestation -> Nullable<Binary>,
        status -> Nullable<Text>,
    }
}

diesel::table! {
    rebuilds (id) {
        id -> Integer,
        build_input_id -> Integer,
        started_at -> Nullable<Timestamp>,
        built_at -> Nullable<Timestamp>,
        build_log -> Binary,
        status -> Nullable<Text>,
    }
}

diesel::table! {
    source_packages (id) {
        id -> Integer,
        name -> Text,
        version -> Text,
        distribution -> Text,
        release -> Nullable<Text>,
        component -> Nullable<Text>,
    }
}

diesel::table! {
    workers (id) {
        id -> Integer,
        name -> Text,
        key -> Text,
        address -> Text,
        status -> Nullable<Text>,
        last_ping -> Timestamp,
        online -> Bool,
    }
}

diesel::joinable!(binary_packages -> build_inputs (build_input_id));
diesel::joinable!(binary_packages -> source_packages (source_package_id));
diesel::joinable!(build_inputs -> source_packages (source_package_id));
diesel::joinable!(queue -> build_inputs (build_input_id));
diesel::joinable!(rebuild_artifacts -> rebuilds (rebuild_id));
diesel::joinable!(rebuilds -> build_inputs (build_input_id));

diesel::allow_tables_to_appear_in_same_query!(
    binary_packages,
    build_inputs,
    queue,
    rebuild_artifacts,
    rebuilds,
    source_packages,
    workers,
);
