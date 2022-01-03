-- Create anime table
CREATE TABLE anime
(
    id                 INT PRIMARY KEY,
    slug               VARCHAR(255) NOT NULL,
    synopsis           TEXT,
    description        TEXT,
    abbreviated_titles TEXT[],
    rating             REAL,
    show_type          VARCHAR(8),
    sub_type           VARCHAR(8),
    episode_count      SMALLINT
);

-- Create anime titles table
CREATE TABLE anime_titles
(
    anime_id INT,
    language VARCHAR(8),
    title    VARCHAR(512) NOT NULL,
    PRIMARY KEY (anime_id, language),
    FOREIGN KEY (anime_id) REFERENCES anime (id) ON DELETE CASCADE ON UPDATE CASCADE
);


-- Create Anime Images table
CREATE TABLE anime_images
(
    anime_id   INT,
    image_type VARCHAR(16),
    size       TEXT,
    url        VARCHAR(1024) NOT NULL,
    PRIMARY KEY (anime_id, image_type, size),
    FOREIGN KEY (anime_id) REFERENCES anime (id) ON DELETE CASCADE ON UPDATE CASCADE
);