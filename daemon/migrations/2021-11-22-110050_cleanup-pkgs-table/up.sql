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
    CONSTRAINT packages_unique UNIQUE (name, distro, suite, architecture),
    FOREIGN KEY(pkgbase_id) REFERENCES pkgbases(id) ON DELETE CASCADE,
    FOREIGN KEY(build_id) REFERENCES builds(id) ON DELETE SET NULL
);

INSERT INTO _packages_new (id, pkgbase_id, name, version, status, distro, suite, architecture, artifact_url, build_id, built_at, has_diffoscope, has_attestation, checksum)
    SELECT id, base_id, name, version, status, distro, suite, architecture, artifact_url, build_id, built_at, has_diffoscope, has_attestation, checksum
    FROM packages
    WHERE base_id IS NOT NULL;

DROP TABLE packages;
ALTER TABLE _packages_new RENAME TO packages;

PRAGMA foreign_keys=on;

-- drop all packages that we still need to build so they get properly re-initialized next sync
delete from pkgbases where id in (select pkgbase_id from packages where status != 'GOOD') and artifacts='[]';
