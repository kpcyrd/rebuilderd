DROP TABLE queue;

CREATE TABLE queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    pkgbase_id INTEGER NOT NULL,
    version VARCHAR NOT NULL,
    required_backend VARCHAR NOT NULL,
    priority INTEGER NOT NULL,
    queued_at DATETIME NOT NULL,
    worker_id INTEGER,
    started_at DATETIME,
    last_ping DATETIME,
    FOREIGN KEY(pkgbase_id) REFERENCES pkgbases(id) ON DELETE CASCADE,
    FOREIGN KEY(worker_id) REFERENCES workers(id) ON DELETE SET NULL,
    CONSTRAINT queue_unique UNIQUE (pkgbase_id, version)
);

CREATE UNIQUE INDEX queue_pop_idx ON queue(required_backend, priority, queued_at, id);

PRAGMA foreign_keys=off;

CREATE TABLE _pkgbases_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name VARCHAR NOT NULL,
    version VARCHAR NOT NULL,
    distro VARCHAR NOT NULL,
    suite VARCHAR NOT NULL,
    architecture VARCHAR NOT NULL,
    input_url VARCHAR,
    artifacts VARCHAR NOT NULL,
    retries INTEGER NOT NULL,
    next_retry DATETIME,
    CONSTRAINT pkgbase_unique UNIQUE (name, version, distro, suite, architecture)
);

INSERT INTO _pkgbases_new (id, name, version, distro, suite, architecture, artifacts, retries, next_retry)
    SELECT id, name, version, distro, suite, architecture, "[]", retries, next_retry
    FROM pkgbases;

DROP TABLE pkgbases;
ALTER TABLE _pkgbases_new RENAME TO pkgbases;

PRAGMA foreign_keys=on;
