CREATE TABLE tags
(
    id  INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    tag TEXT    NOT NULL
);

CREATE UNIQUE INDEX tags_unique_idx ON tags (tag);

CREATE TABLE worker_tags
(
    worker_id INTEGER NOT NULL REFERENCES workers,
    tag_id    INTEGER NOT NULL REFERENCES tags,

    PRIMARY KEY (worker_id, tag_id)
);

CREATE INDEX worker_tags_worker_id_idx ON worker_tags (worker_id);
CREATE INDEX worker_tags_tag_id_idx ON worker_tags (tag_id);

CREATE TABLE source_package_tag_rules
(
    id                             INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    tag_id                         INTEGER NOT NULL REFERENCES tags,
    source_package_name_pattern    TEXT    NOT NULL,
    source_package_version_pattern TEXT
);

CREATE UNIQUE INDEX source_package_tag_rules_unique_idx ON source_package_tag_rules (tag_id,
                                                                                     source_package_name_pattern,
                                                                                     COALESCE(source_package_version_pattern, 'PLACEHOLDER'));

CREATE INDEX source_package_tag_rules_tag_id_idx ON source_package_tag_rules (tag_id);