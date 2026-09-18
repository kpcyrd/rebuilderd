CREATE TABLE stats
(
    id           INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
    captured_at  DATETIME NOT NULL,
    distribution TEXT,
    release      TEXT,
    architecture TEXT,
    good         INTEGER  NOT NULL DEFAULT 0,
    bad          INTEGER  NOT NULL DEFAULT 0,
    fail         INTEGER  NOT NULL DEFAULT 0,
    unknown      INTEGER  NOT NULL DEFAULT 0
);

CREATE INDEX stats_captured_at_idx ON stats (captured_at);
CREATE INDEX stats_distribution_idx ON stats (distribution);
CREATE INDEX stats_release_idx ON stats ("release");
CREATE INDEX stats_architecture_idx ON stats (architecture);
