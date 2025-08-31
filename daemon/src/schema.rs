// @generated automatically by Diesel CLI.

diesel::table! {
    attestation_logs (id) {
        id -> Integer,
        attestation_log -> Binary,
    }
}

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
    build_logs (id) {
        id -> Integer,
        build_log -> Binary,
    }
}

diesel::table! {
    diffoscope_logs (id) {
        id -> Integer,
        diffoscope_log -> Binary,
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
        diffoscope_log_id -> Nullable<Integer>,
        attestation_log_id -> Nullable<Integer>,
        status -> Nullable<Text>,
    }
}

diesel::table! {
    rebuilds (id) {
        id -> Integer,
        build_input_id -> Integer,
        started_at -> Nullable<Timestamp>,
        built_at -> Nullable<Timestamp>,
        build_log_id -> Integer,
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
        last_seen -> Timestamp,
        seen_in_last_sync -> Bool
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
diesel::joinable!(rebuild_artifacts -> attestation_logs (attestation_log_id));
diesel::joinable!(rebuild_artifacts -> diffoscope_logs (diffoscope_log_id));
diesel::joinable!(rebuild_artifacts -> rebuilds (rebuild_id));
diesel::joinable!(rebuilds -> build_inputs (build_input_id));
diesel::joinable!(rebuilds -> build_logs (build_log_id));

diesel::allow_tables_to_appear_in_same_query!(
    attestation_logs,
    binary_packages,
    build_inputs,
    build_logs,
    diffoscope_logs,
    queue,
    rebuild_artifacts,
    rebuilds,
    source_packages,
    workers,
);
