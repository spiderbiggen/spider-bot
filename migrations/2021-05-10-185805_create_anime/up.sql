-- Create anime table
CREATE TABLE anime
(
    id                 INT PRIMARY KEY,
    slug               VARCHAR(255) NOT NULL,
    synopsis           TEXT         NOT NULL,
    description        TEXT         NOT NULL,
    abbreviated_titles TEXT[],
    rating             DECIMAL(2),
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
CREATE TABLE anime_images(

);