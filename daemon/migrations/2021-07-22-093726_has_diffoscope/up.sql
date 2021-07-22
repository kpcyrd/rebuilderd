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
    SELECT id, base_id, name, version, status, distro, suite, architecture, url, build_id, built_at, false, attestation, checksum, retries, next_retry
    FROM packages;

DROP TABLE packages;
ALTER TABLE _packages_new RENAME TO packages;

PRAGMA foreign_keys=on;

-- initialize new has_diffoscope field
update packages set has_diffoscope=true where build_id in (select id from builds where diffoscope is not null);
