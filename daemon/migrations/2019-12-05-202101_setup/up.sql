CREATE TABLE packages (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name VARCHAR NOT NULL,
    version VARCHAR NOT NULL,
    status VARCHAR NOT NULL,
    distro VARCHAR NOT NULL,
    suite VARCHAR NOT NULL,
    architecture VARCHAR NOT NULL,
    url VARCHAR NOT NULL,
    CONSTRAINT packages_unique UNIQUE (name, distro, suite, architecture)
);

CREATE TABLE queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    package_id INTEGER NOT NULL,
    queued_at DATETIME NOT NULL,
    worker_id INTEGER,
    started_at DATETIME,
    last_ping DATETIME,
    FOREIGN KEY(package_id) REFERENCES packages(id),
    FOREIGN KEY(worker_id) REFERENCES workers(id),
    CONSTRAINT queue_unique UNIQUE (package_id)
);

CREATE TABLE workers (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    key VARCHAR NOT NULL,
    addr VARCHAR NOT NULL,
    status VARCHAR,
    last_ping TIMESTAMP NOT NULL,
    online BOOLEAN NOT NULL,
    CONSTRAINT workers_unique UNIQUE (key)
);
