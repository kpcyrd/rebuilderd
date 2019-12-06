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

-- CREATE TABLE queue (
--     id SERIAL PRIMARY KEY,
-- );

CREATE TABLE workers (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    key VARCHAR NOT NULL,
    addr VARCHAR NOT NULL,
    status VARCHAR,
    last_ping TIMESTAMP NOT NULL,
    online BOOLEAN NOT NULL,
    CONSTRAINT workers_unique UNIQUE (key)
);
