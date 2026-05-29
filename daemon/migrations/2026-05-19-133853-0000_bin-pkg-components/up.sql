PRAGMA foreign_keys= OFF;

CREATE TABLE _new_binary_packages
(
    id                  INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    source_package_id   INTEGER NOT NULL REFERENCES source_packages ON DELETE CASCADE,
    build_input_id      INTEGER NOT NULL REFERENCES build_inputs ON DELETE CASCADE,
    name                TEXT    NOT NULL,
    version             TEXT    NOT NULL,
    component           TEXT,
    architecture        TEXT    NOT NULL,
    artifact_url        TEXT    NOT NULL
);

INSERT INTO _new_binary_packages (id, source_package_id, build_input_id, name, version, component, architecture, artifact_url)
SELECT b.id,
       b.source_package_id,
       b.build_input_id,
       b.name,
       b.version,
       s.component,
       b.architecture,
       b.artifact_url
FROM binary_packages b
LEFT JOIN source_packages s ON b.source_package_id = s.id;

DROP TABLE binary_packages;
ALTER TABLE _new_binary_packages
    RENAME TO binary_packages;

CREATE UNIQUE INDEX binary_packages_unique_idx ON binary_packages (source_package_id, build_input_id, name, version, architecture);
CREATE INDEX binary_packages_source_packages_id_idx ON binary_packages (source_package_id);
CREATE INDEX binary_packages_build_input_id_idx ON binary_packages (build_input_id);
CREATE INDEX binary_packages_name_idx ON binary_packages (name);
CREATE INDEX binary_packages_architecture_idx ON binary_packages (architecture);

PRAGMA foreign_keys= ON;
