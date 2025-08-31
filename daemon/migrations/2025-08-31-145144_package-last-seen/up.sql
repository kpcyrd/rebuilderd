PRAGMA foreign_keys= OFF;

CREATE TABLE _new_source_packages
(
    id                INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
    name              TEXT     NOT NULL,
    version           TEXT     NOT NULL,
    distribution      TEXT     NOT NULL,
    "release"         TEXT,
    component         TEXT,
    last_seen         DATETIME NOT NULL,
    seen_in_last_sync BOOLEAN  NOT NULL
);

INSERT INTO _new_source_packages (id, name, version, distribution, "release", component, last_seen, seen_in_last_sync)
SELECT id,
       name,
       version,
       distribution,
       "release",
       component,
       DATETIME('now'),
       TRUE
FROM source_packages;

DROP TABLE source_packages;
ALTER TABLE _new_source_packages
    RENAME TO source_packages;

CREATE UNIQUE INDEX source_packages_unique_idx ON source_packages (name, version, distribution,
                                                                   COALESCE("release", 'PLACEHOLDER'),
                                                                   COALESCE(component, 'PLACEHOLDER'));

CREATE INDEX source_packages_name_idx ON source_packages (name);
CREATE INDEX source_packages_distribution_idx ON source_packages (distribution);
CREATE INDEX source_packages_release_idx ON source_packages ("release");
CREATE INDEX source_packages_component_idx ON source_packages (component);
CREATE INDEX source_packages_last_seen_idx ON source_packages (last_seen);
CREATE INDEX source_packages_seen_in_last_sync_idx ON source_packages (seen_in_last_sync);

PRAGMA foreign_keys= ON;