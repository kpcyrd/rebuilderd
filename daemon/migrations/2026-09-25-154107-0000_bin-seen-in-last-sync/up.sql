PRAGMA foreign_keys=OFF;

CREATE TABLE _new_binary_packages
(
    id                  INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
    source_package_id   INTEGER  NOT NULL REFERENCES source_packages ON DELETE CASCADE,
    build_input_id      INTEGER  NOT NULL REFERENCES build_inputs ON DELETE CASCADE,
    name                TEXT     NOT NULL,
    version             TEXT     NOT NULL,
    component           TEXT,
    architecture        TEXT     NOT NULL,
    artifact_url        TEXT     NOT NULL,
    last_seen           DATETIME NOT NULL,
    seen_in_last_sync   BOOLEAN  NOT NULL
);

INSERT INTO _new_binary_packages (id, source_package_id, build_input_id, name, version, component, architecture, artifact_url, last_seen, seen_in_last_sync)
SELECT b.id,
       b.source_package_id,
       b.build_input_id,
       b.name,
       b.version,
       b.component,
       b.architecture,
       b.artifact_url,
       s.last_seen,
       s.seen_in_last_sync
FROM binary_packages b
LEFT JOIN source_packages s ON b.source_package_id = s.id;

DROP TABLE binary_packages;
ALTER TABLE _new_binary_packages
    RENAME TO binary_packages;

CREATE INDEX binary_packages_source_packages_id_idx ON binary_packages (source_package_id);
CREATE INDEX binary_packages_build_input_id_idx ON binary_packages (build_input_id);
CREATE INDEX binary_packages_name_idx ON binary_packages (name);
CREATE INDEX binary_packages_architecture_idx ON binary_packages (architecture);
CREATE INDEX binary_packages_last_seen_idx ON binary_packages (last_seen);
CREATE INDEX binary_packages_seen_in_last_sync_idx ON binary_packages (seen_in_last_sync);
CREATE UNIQUE INDEX binary_packages_unique_idx ON binary_packages (source_package_id, build_input_id, name, version, component, architecture);

PRAGMA foreign_keys=ON;
