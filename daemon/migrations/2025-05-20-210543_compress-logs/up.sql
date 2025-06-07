PRAGMA foreign_keys=off;

CREATE TABLE _builds_new
(
    id          INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    diffoscope  BLOB,
    build_log   BLOB    NOT NULL,
    attestation BLOB
);

INSERT INTO _builds_new(id, diffoscope, build_log, attestation)
SELECT id, diffoscope, build_log, attestation
FROM builds;

DROP TABLE builds;

ALTER TABLE _builds_new
    RENAME TO builds;

PRAGMA foreign_keys=on;
