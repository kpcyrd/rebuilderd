CREATE TABLE tags
(
    id  INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    tag TEXT    NOT NULL
);

CREATE UNIQUE INDEX tags_unique_idx ON tags (tag);

CREATE TABLE worker_tags
(
    worker_id INTEGER NOT NULL REFERENCES workers ON DELETE CASCADE,
    tag_id    INTEGER NOT NULL REFERENCES tags ON DELETE CASCADE,

    PRIMARY KEY (worker_id, tag_id)
);

CREATE INDEX worker_tags_worker_id_idx ON worker_tags (worker_id);
CREATE INDEX worker_tags_tag_id_idx ON worker_tags (tag_id);

CREATE TABLE tag_rules
(
    id              INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    tag_id          INTEGER NOT NULL REFERENCES tags ON DELETE CASCADE,
    name_pattern    TEXT    NOT NULL,
    version_pattern TEXT
);

CREATE UNIQUE INDEX tag_rules_unique_idx ON tag_rules (tag_id,
                                                       name_pattern,
                                                       COALESCE(version_pattern, 'PLACEHOLDER'));

CREATE INDEX tag_rules_tag_id_idx ON tag_rules (tag_id);