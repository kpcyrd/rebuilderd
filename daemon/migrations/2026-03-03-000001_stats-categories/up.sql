CREATE TABLE stats_categories
(
    id       INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    stats_id INTEGER NOT NULL REFERENCES stats (id),
    category TEXT    NOT NULL,
    count    INTEGER NOT NULL
);

CREATE INDEX stats_categories_stats_id_idx ON stats_categories (stats_id);
