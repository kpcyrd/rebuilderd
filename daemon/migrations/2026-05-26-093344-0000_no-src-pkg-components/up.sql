PRAGMA foreign_keys= OFF;

CREATE TABLE _new_source_packages
(
    id                INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
    name              TEXT     NOT NULL,
    version           TEXT     NOT NULL,
    distribution      TEXT     NOT NULL,
    "release"         TEXT,
    last_seen         DATETIME NOT NULL,
    seen_in_last_sync BOOLEAN  NOT NULL
);

-- NOTE: this now has a stricter unique constraint, you may have to drop e.g. `main/installer` pkgs for this to work
INSERT INTO _new_source_packages (id, name, version, distribution, "release", last_seen, seen_in_last_sync)
SELECT id,
       name,
       version,
       distribution,
       "release",
       last_seen,
       seen_in_last_sync
FROM source_packages;

DROP TABLE source_packages;
ALTER TABLE _new_source_packages
    RENAME TO source_packages;

CREATE UNIQUE INDEX source_packages_unique_idx ON source_packages (name, version, distribution,
                                                                   COALESCE("release", 'PLACEHOLDER'));
CREATE INDEX source_packages_name_idx ON source_packages (name);
CREATE INDEX source_packages_distribution_idx ON source_packages (distribution);
CREATE INDEX source_packages_release_idx ON source_packages ("release");
CREATE INDEX source_packages_last_seen_idx ON source_packages (last_seen);
CREATE INDEX source_packages_seen_in_last_sync_idx ON source_packages (seen_in_last_sync);

-- Fix unique index on binary_packages to allow binary package to exist in multiple components
DROP INDEX binary_packages_unique_idx;
CREATE UNIQUE INDEX binary_packages_unique_idx ON binary_packages (source_package_id, build_input_id, name, version, component, architecture);

PRAGMA foreign_keys= ON;
