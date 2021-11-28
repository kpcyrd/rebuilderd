PRAGMA foreign_keys=off;

CREATE TABLE _packages_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    pkgbase_id INTEGER NOT NULL,
    name VARCHAR NOT NULL,
    version VARCHAR NOT NULL,
    status VARCHAR NOT NULL,
    distro VARCHAR NOT NULL,
    suite VARCHAR NOT NULL,
    architecture VARCHAR NOT NULL,
    artifact_url VARCHAR NOT NULL,
    build_id INTEGER,
    built_at DATETIME,
    has_diffoscope BOOLEAN NOT NULL,
    has_attestation BOOLEAN NOT NULL,
    checksum VARCHAR,
    CONSTRAINT packages_unique UNIQUE (name, version, distro, suite, architecture),
    FOREIGN KEY(pkgbase_id) REFERENCES pkgbases(id) ON DELETE CASCADE,
    FOREIGN KEY(build_id) REFERENCES builds(id) ON DELETE SET NULL
);

INSERT INTO _packages_new (id, pkgbase_id, name, version, status, distro, suite, architecture, artifact_url, build_id, built_at, has_diffoscope, has_attestation, checksum)
    SELECT id, pkgbase_id, name, version, status, distro, suite, architecture, artifact_url, build_id, built_at, has_diffoscope, has_attestation, checksum
    FROM packages;

DROP TABLE packages;
ALTER TABLE _packages_new RENAME TO packages;

PRAGMA foreign_keys=on;

CREATE INDEX packages_pkgbase_idx ON packages(pkgbase_id);
CREATE INDEX queue_pkgbase_idx ON queue(pkgbase_id);
