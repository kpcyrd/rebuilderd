PRAGMA foreign_keys= OFF;

-- source_packages

CREATE TABLE source_packages
(
    id           INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name         TEXT    NOT NULL,
    version      TEXT    NOT NULL,
    distribution TEXT    NOT NULL,
    "release"    TEXT,
    component    TEXT
);


CREATE UNIQUE INDEX source_packages_unique_idx ON source_packages (name, version, distribution,
                                                                   COALESCE("release", 'PLACEHOLDER'),
                                                                   COALESCE(component, 'PLACEHOLDER'));

-- "release" has not been stored in the database up until this point. Users will need to post-process their databases
-- when merging into a single file
INSERT INTO source_packages(id, name, version, distribution, component)
SELECT id,
       name,
       version,
       distro,
       suite
FROM pkgbases;

CREATE INDEX source_packages_name_idx ON source_packages (name);
CREATE INDEX source_packages_distribution_idx ON source_packages (distribution);
CREATE INDEX source_packages_release_idx ON source_packages ("release");
CREATE INDEX source_packages_component_idx ON source_packages (component);

-- build_inputs

CREATE TABLE build_inputs
(
    id                INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    source_package_id INTEGER NOT NULL REFERENCES source_packages ON DELETE CASCADE,
    url               TEXT    NOT NULL,
    backend           TEXT    NOT NULL,
    architecture      TEXT    NOT NULL,
    retries           INTEGER NOT NULL,
    next_retry        DATETIME
);

CREATE UNIQUE INDEX build_inputs_unique_idx ON build_inputs (source_package_id, url, backend, architecture);

-- synthesize build inputs from the old pkgbases table using the distro as the backend
INSERT INTO build_inputs(source_package_id, url, backend, architecture, retries, next_retry)
SELECT id, input_url, distro, architecture, retries, next_retry
FROM pkgbases;

CREATE INDEX build_inputs_source_package_id_idx ON build_inputs (source_package_id);
CREATE INDEX build_inputs_backend_idx ON build_inputs (backend);
CREATE INDEX build_inputs_architecture_idx ON build_inputs (architecture);
CREATE INDEX build_inputs_next_retry_idx ON build_inputs (next_retry);

-- binary_packages

CREATE TABLE binary_packages
(
    id                INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    source_package_id INTEGER NOT NULL REFERENCES source_packages ON DELETE CASCADE,
    build_input_id    INTEGER NOT NULL REFERENCES build_inputs ON DELETE CASCADE,
    name              TEXT    NOT NULL,
    version           TEXT    NOT NULL,
    architecture      TEXT    NOT NULL,
    artifact_url      TEXT    NOT NULL
);

CREATE UNIQUE INDEX binary_packages_unique_idx ON binary_packages (source_package_id, build_input_id, name, version, architecture);

INSERT INTO binary_packages(id, source_package_id, build_input_id, name, version, architecture, artifact_url)
SELECT packages.id,
       packages.pkgbase_id,
       build_inputs.id,
       packages.name,
       packages.version,
       packages.architecture,
       packages.artifact_url
FROM packages
         INNER JOIN build_inputs ON packages.pkgbase_id = build_inputs.source_package_id;

CREATE INDEX binary_packages_source_packages_id_idx ON binary_packages (source_package_id);
CREATE INDEX binary_packages_build_input_id_idx ON binary_packages (build_input_id);
CREATE INDEX binary_packages_name_idx ON binary_packages (name);
CREATE INDEX binary_packages_architecture_idx ON binary_packages (architecture);

-- build logs
CREATE TABLE build_logs
(
    id        INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    build_log BLOB    NOT NULL
);

-- build logs may be null in the old schema. We don't want that, but we also need to preserve backwards compatibility.
-- insert a zero-length zstd stream in the place of any null build logs.
INSERT INTO build_logs(build_log)
SELECT COALESCE(builds.build_log, x'28b52ffd240001000099e9d851')
FROM builds;

CREATE INDEX build_logs_build_log_idx ON build_logs (build_log);

CREATE TABLE diffoscope_logs
(
    id             INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    diffoscope_log BLOB    NOT NULL
);

INSERT INTO diffoscope_logs(diffoscope_log)
SELECT diffoscope
FROM builds
WHERE diffoscope IS NOT NULL;

CREATE INDEX diffoscope_logs_diffoscope_log_idx ON diffoscope_logs (diffoscope_log);

CREATE TABLE attestation_logs
(
    id              INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    attestation_log BLOB    NOT NULL
);

INSERT INTO attestation_logs(attestation_log)
SELECT attestation
FROM builds
WHERE attestation IS NOT NULL;

CREATE INDEX attestation_logs_attestation_log_idx ON attestation_logs (attestation_log);

-- rebuilds
CREATE TABLE rebuilds
(
    id             INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    build_input_id INTEGER NOT NULL REFERENCES build_inputs ON DELETE CASCADE,
    started_at     DATETIME,
    built_at       DATETIME,
    build_log_id   INTEGER NOT NULL REFERENCES build_logs ON DELETE CASCADE,
    status         TEXT
);

INSERT INTO rebuilds(build_input_id, started_at, built_at, build_log_id, status)

SELECT build_inputs.id,
       NULL,
       MAX(packages.built_at),
       (SELECT id FROM build_logs WHERE build_log = COALESCE(builds.build_log, x'28b52ffd240001000099e9d851')),
       MIN(packages.status)
FROM build_inputs
         INNER JOIN packages ON build_inputs.source_package_id = packages.pkgbase_id
         LEFT OUTER JOIN builds ON packages.build_id = builds.id
GROUP BY build_inputs.id;

CREATE INDEX rebuilds_build_input_id_idx ON rebuilds (build_input_id);
CREATE INDEX rebuilds_started_at_idx ON rebuilds (started_at);
CREATE INDEX rebuilds_built_at_id_idx ON rebuilds (built_at);
CREATE INDEX rebuilds_status_idx ON rebuilds (status);

CREATE TABLE rebuild_artifacts
(
    id                 INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    rebuild_id         INTEGER NOT NULL REFERENCES rebuilds ON DELETE CASCADE,
    name               TEXT    NOT NULL,
    diffoscope_log_id  INTEGER REFERENCES diffoscope_logs ON DELETE SET NULL,
    attestation_log_id INTEGER REFERENCES attestation_logs ON DELETE SET NULL,
    status             TEXT
);

INSERT INTO rebuild_artifacts(rebuild_id, name, diffoscope_log_id, attestation_log_id, status)
SELECT rebuilds.id,
       packages.name,
       (SELECT id FROM diffoscope_logs WHERE diffoscope_log = builds.diffoscope),
       (SELECT id FROM attestation_logs WHERE attestation_log = builds.attestation),
       packages.status
FROM packages
         INNER JOIN (rebuilds INNER JOIN build_inputs ON rebuilds.build_input_id = build_inputs.id)
                    ON packages.pkgbase_id = build_inputs.source_package_id
         INNER JOIN builds ON packages.build_id = builds.id;

CREATE INDEX rebuild_artifacts_rebuild_id_idx ON rebuild_artifacts (rebuild_id);
CREATE INDEX rebuild_artifacts_status_idx ON rebuild_artifacts (status);

-- queue
CREATE TABLE _queue_new
(
    id             INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
    build_input_id INTEGER  NOT NULL REFERENCES build_inputs ON DELETE CASCADE,
    priority       INTEGER  NOT NULL,
    queued_at      DATETIME NOT NULL,
    started_at     DATETIME,
    worker         INTEGER,
    last_ping      DATETIME
);

CREATE UNIQUE INDEX queue_unique_idx ON _queue_new (build_input_id);

INSERT INTO _queue_new(id, build_input_id, priority, queued_at, started_at, worker, last_ping)
SELECT queue.id, build_inputs.id, priority, queued_at, started_at, worker_id, last_ping
FROM queue
         INNER JOIN build_inputs ON build_inputs.source_package_id = queue.pkgbase_id;

DROP TABLE queue;
ALTER TABLE _queue_new
    RENAME TO queue;

CREATE INDEX queue_priority_idx ON queue (priority);
CREATE INDEX queue_queued_at_idx ON queue (queued_at);
CREATE INDEX queue_last_ping_idx ON queue (last_ping);

-- workers
CREATE TABLE _workers_new
(
    id        INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
    name      TEXT     NOT NULL,
    key       TEXT     NOT NULL,
    address   TEXT     NOT NULL,
    status    TEXT,
    last_ping DATETIME NOT NULL,
    online    BOOLEAN  NOT NULL
);

CREATE UNIQUE INDEX workers_unique_idx ON _workers_new (key);

INSERT INTO _workers_new(id, name, key, address, status, last_ping, online)
SELECT id, '', key, addr, status, last_ping, online
FROM workers;

DROP TABLE workers;
ALTER TABLE _workers_new
    RENAME TO workers;

CREATE INDEX workers_last_ping_idx ON workers (last_ping);
CREATE INDEX workers_online_idx ON workers (online);

DROP TABLE builds;
DROP TABLE packages;
DROP TABLE pkgbases;

PRAGMA foreign_keys= ON;
