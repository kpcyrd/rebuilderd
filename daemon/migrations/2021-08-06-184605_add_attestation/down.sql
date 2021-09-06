-- add attestation & drop has_attestation column in "packages"
PRAGMA foreign_keys=off;

CREATE TABLE _packages_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    base_id INTEGER,
    name VARCHAR NOT NULL,
    version VARCHAR NOT NULL,
    status VARCHAR NOT NULL,
    distro VARCHAR NOT NULL,
    suite VARCHAR NOT NULL,
    architecture VARCHAR NOT NULL,
    url VARCHAR NOT NULL,
    build_id INTEGER,
    built_at DATETIME,
    has_diffoscope BOOLEAN NOT NULL,
    attestation VARCHAR,
    checksum VARCHAR,
    retries INTEGER NOT NULL,
    next_retry DATETIME,
    CONSTRAINT packages_unique UNIQUE (name, distro, suite, architecture),
    FOREIGN KEY(base_id) REFERENCES pkgbases(id),
    FOREIGN KEY(build_id) REFERENCES builds(id)
);

INSERT INTO _packages_new (id, base_id, name, version, status, distro, suite, architecture, url, build_id, built_at, has_diffoscope, attestation, checksum, retries, next_retry)
    SELECT id, base_id, name, version, status, distro, suite, architecture, url, build_id, built_at, false, NULL, checksum, retries, next_retry
    FROM packages;

-- copy all attestations values from "build" to "packages" table
UPDATE _packages_new
  SET attestation=(
        SELECT attestation
        FROM builds
        WHERE builds.id = _packages_new.build_id
    );

DROP TABLE packages;
ALTER TABLE _packages_new RENAME TO packages;

PRAGMA foreign_keys=on;

-- drop attestation column in "build"
PRAGMA foreign_keys=off;

CREATE TABLE _builds_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    diffoscope TEXT,
    build_log BLOB NOT NULL
);

INSERT INTO _builds_new (id, diffoscope, build_log)
    SELECT id, diffoscope, build_log
    FROM builds;

DROP TABLE builds;
ALTER TABLE _builds_new RENAME TO builds;

PRAGMA foreign_keys=on;
