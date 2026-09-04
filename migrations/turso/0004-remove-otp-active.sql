CREATE TABLE IF NOT EXISTS new_users (
    "id" BLOB NOT NULL,
    "nickname" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "password" TEXT NOT NULL,
    "preferred_language" TEXT NOT NULL,
    "permission" INTEGER NOT NULL,
    "totp_secret" TEXT,
    PRIMARY KEY ("id")
);

INSERT INTO new_users (id, nickname, email, password, preferred_language, permission, totp_secret)
SELECT id, nickname, email, password, preferred_language, permission, totp_secret
FROM users;

DROP TABLE users;

ALTER TABLE new_users RENAME TO users;