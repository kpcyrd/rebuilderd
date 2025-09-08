--
-- This script merges a rebuilderd database into another. Useful for combining
-- databases that were previously separated due to schema constraints
-- (architectures, releases, different distros, etc).
--

ATTACH DATABASE :OTHER_DB AS other;
BEGIN TRANSACTION;

-- source packages
INSERT INTO source_packages (name, version, distribution, "release", component, last_seen, seen_in_last_sync)
SELECT name, version, distribution, "release", component, last_seen, seen_in_last_sync
FROM other.source_packages
WHERE TRUE
ON CONFLICT DO NOTHING;

-- build inputs, mapped to source packages
INSERT INTO build_inputs (source_package_id, url, backend, architecture, retries, next_retry)
SELECT (SELECT id
        FROM source_packages
        WHERE (name, version, distribution, "release", component) IS
              (SELECT name, version, distribution, "release", component
               FROM other.source_packages
               WHERE id = other.build_inputs.source_package_id)),
       url,
       backend,
       architecture,
       retries,
       next_retry
FROM other.build_inputs
WHERE TRUE
ON CONFLICT DO NOTHING;

-- binary packages, mapped to source packages and build inputs
INSERT INTO binary_packages (source_package_id, build_input_id, name, version, architecture, artifact_url)
SELECT (SELECT id
        FROM source_packages
        WHERE (name, version, distribution, "release", component) IS
              (SELECT name, version, distribution, "release", component
               FROM other.source_packages
               WHERE id = other.binary_packages.source_package_id)),
       (SELECT id
        FROM build_inputs
        WHERE source_package_id IS (SELECT id
                                    FROM source_packages
                                    WHERE (name, version, distribution, "release", component) IS
                                          (SELECT name, version, distribution, "release", component
                                           FROM other.source_packages
                                           WHERE id = other.binary_packages.source_package_id))),
       name,
       version,
       architecture,
       artifact_url
FROM other.binary_packages
WHERE TRUE
ON CONFLICT DO NOTHING;

-- build logs, with a temporary map to keep track of which is which
CREATE TEMPORARY TABLE _build_log_map
(
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    old_id INTEGER NOT NULL,
    new_id INTEGER
)

INSERT INTO _build_log_map(old_id, new_id)
SELECT id, (SELECT seq FROM main.sqlite_sequence WHERE name IS 'build_logs') + rowid
FROM other.build_logs
ORDER BY id;

INSERT INTO build_logs(build_log)
SELECT build_log
FROM other.build_logs
ORDER BY id;

CREATE INDEX _temp_build_log_id_index ON _build_log_map (old_id);

-- attestation logs, with a temporary map to keep track of which is which
CREATE TEMPORARY TABLE _attestation_log_map
(
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    old_id INTEGER NOT NULL,
    new_id INTEGER
);

INSERT INTO _attestation_log_map(old_id, new_id)
SELECT id, (SELECT seq FROM main.sqlite_sequence WHERE name IS 'attestation_logs') + rowid
FROM other.attestation_logs
ORDER BY id;

INSERT INTO attestation_logs(attestation_log)
SELECT attestation_log
FROM other.attestation_logs
ORDER BY id;

CREATE INDEX _temp_attestation_id_index ON _attestation_log_map (old_id);

-- diffoscope logs, with a temporary map to keep track of which is which
CREATE TEMPORARY TABLE _diffoscope_log_map
(
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    old_id INTEGER NOT NULL,
    new_id INTEGER
);

INSERT INTO _diffoscope_log_map(old_id, new_id)
SELECT id, (SELECT seq FROM main.sqlite_sequence WHERE name IS 'diffoscope_logs') + rowid
FROM other.diffoscope_logs
ORDER BY id;

INSERT INTO diffoscope_logs(diffoscope_log)
SELECT diffoscope_log
FROM other.diffoscope_logs
ORDER BY id;

CREATE INDEX _temp_diffoscope_id_index ON _diffoscope_log_map (old_id);

-- rebuilds, with a temporary map to keep track of which is which
CREATE TEMPORARY TABLE _rebuilds_map
(
    id     INTEGER PRIMARY KEY AUTOINCREMENT,
    old_id INTEGER NOT NULL,
    new_id INTEGER
);
INSERT INTO _rebuilds_map(old_id, new_id)
SELECT id, (SELECT seq FROM main.sqlite_sequence WHERE name IS 'rebuilds') + rowid
FROM other.rebuilds
ORDER BY id;

INSERT INTO rebuilds (build_input_id, started_at, built_at, build_log_id, status)
SELECT ( -- build_input_id in the merged db
           SELECT build_inputs.id
           FROM build_inputs
                    INNER JOIN source_packages ON build_inputs.source_package_id = source_packages.id
           WHERE (name, version, distribution, "release", component) IS
                 (SELECT name, version, distribution, "release", component
                  FROM other.source_packages
                           INNER JOIN other.build_inputs
                                      ON other.source_packages.id IS other.build_inputs.source_package_id
                  WHERE other.build_inputs.id = other.rebuilds.build_input_id)),
       started_at,
       built_at,
       (SELECT new_id FROM _build_log_map WHERE old_id IS other.rebuilds.build_log_id),
       status
FROM other.rebuilds
WHERE TRUE
ON CONFLICT DO NOTHING;

-- rebuild artifacts
INSERT INTO rebuild_artifacts(rebuild_id, name, diffoscope_log_id, attestation_log_id, status)
SELECT (SELECT new_id FROM _rebuilds_map WHERE old_id IS other.rebuild_artifacts.rebuild_id),
       name,
       (SELECT new_id FROM _diffoscope_log_map WHERE old_id IS other.rebuild_artifacts.diffoscope_log_id),
       (SELECT new_id FROM _attestation_log_map WHERE old_id IS other.rebuild_artifacts.attestation_log_id),
       status
FROM other.rebuild_artifacts
WHERE TRUE
ON CONFLICT DO NOTHING;

COMMIT;
DETACH DATABASE other;
