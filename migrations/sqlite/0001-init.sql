CREATE TABLE IF NOT EXISTS "files" (
    "id" BLOB NOT NULL,
    "created_at" TEXT NOT NULL,
    "hash" TEXT NOT NULL,
    "data" TEXT NOT NULL,
    "ref_count" INTEGER NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE IF NOT EXISTS "game_profiles" (
    "id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "owner_id" BLOB NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE IF NOT EXISTS "profile_textures" (
    "id" BLOB NOT NULL,
    "created_at" TEXT NOT NULL,
    "profile_id" BLOB NOT NULL,
    "skin_model" BIGINT NOT NULL,
    "skin_file" BLOB,
    "cape_file" BLOB,
    PRIMARY KEY ("id")
);
CREATE TABLE IF NOT EXISTS "users" (
    "id" BLOB NOT NULL,
    "nickname" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "password" TEXT NOT NULL,
    "preferred_language" TEXT NOT NULL,
    "permission" INTEGER NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE IF NOT EXISTS "instances" (
    "id" BLOB NOT NULL,
    "name" TEXT NOT NULL,
    "description" TEXT NOT NULL,
    "hash" TEXT NOT NULL,
    "file" BLOB NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE IF NOT EXISTS "user_instances" (
    "id" BLOB NOT NULL,
    "user_id" BLOB NOT NULL,
    "instance_id" BLOB NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE IF NOT EXISTS "tokens" (
    "access_token" BLOB NOT NULL,
    "user_id" BLOB NOT NULL,
    "client_token" TEXT NOT NULL,
    "created_at" TEXT NOT NULL,
    "profile_id" BLOB,
    PRIMARY KEY ("access_token")
);
CREATE UNIQUE INDEX "index_files_by_hash" ON "files" ("hash");
CREATE INDEX "index_game_profiles_by_name" ON "game_profiles" ("name");
CREATE INDEX "index_game_profiles_by_owner_id" ON "game_profiles" ("owner_id");
CREATE INDEX "index_profile_textures_by_profile_id" ON "profile_textures" ("profile_id");
CREATE UNIQUE INDEX "index_users_by_email" ON "users" ("email");
CREATE INDEX "index_user_instances_by_user_id" ON "user_instances" ("user_id");
CREATE INDEX "index_user_instances_by_instance_id" ON "user_instances" ("instance_id");
CREATE INDEX "index_tokens_by_user_id" ON "tokens" ("user_id");
CREATE INDEX "index_tokens_by_profile_id" ON "tokens" ("profile_id");
