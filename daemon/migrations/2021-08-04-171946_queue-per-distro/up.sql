PRAGMA foreign_keys=off;

CREATE TABLE _queue_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    package_id INTEGER NOT NULL,
    version VARCHAR NOT NULL,
    required_backend VARCHAR NOT NULL,
    priority INTEGER NOT NULL,
    queued_at DATETIME NOT NULL,
    worker_id INTEGER,
    started_at DATETIME,
    last_ping DATETIME,
    FOREIGN KEY(package_id) REFERENCES packages(id) ON DELETE CASCADE,
    FOREIGN KEY(worker_id) REFERENCES workers(id) ON DELETE SET NULL,
    CONSTRAINT queue_unique UNIQUE (package_id, version)
);

INSERT INTO _queue_new (id, package_id, version, required_backend, priority, queued_at, worker_id, started_at, last_ping)
    SELECT id, package_id, version, "", priority, queued_at, worker_id, started_at, last_ping
    FROM queue;

DROP TABLE queue;
ALTER TABLE _queue_new RENAME TO queue;

CREATE UNIQUE INDEX queue_pop_idx ON queue(required_backend, priority, queued_at, id);

PRAGMA foreign_keys=on;
