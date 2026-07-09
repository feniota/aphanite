-- Initial database schema

-- File
CREATE TABLE public.files (
    id uuid NOT NULL,
    created_at timestamp(6) with time zone NOT NULL,
    hash text NOT NULL,
    data text NOT NULL,
    ref_count integer NOT NULL
);

ALTER TABLE ONLY public.files
    ADD CONSTRAINT files_pkey PRIMARY KEY (id);
    
CREATE UNIQUE INDEX index_files_by_hash ON public.files USING btree (hash);

-- GameProfile
CREATE TABLE public.game_profiles (
    id uuid NOT NULL,
    name text NOT NULL,
    owner_id uuid NOT NULL
);

ALTER TABLE ONLY public.game_profiles
    ADD CONSTRAINT game_profiles_pkey PRIMARY KEY (id);

-- Instance

CREATE TABLE public.instances (
    id uuid NOT NULL,
    name text NOT NULL,
    description text NOT NULL,
    hash text NOT NULL,
    file uuid NOT NULL
);

ALTER TABLE ONLY public.instances
    ADD CONSTRAINT instances_pkey PRIMARY KEY (id);

-- ProfileTexture

CREATE TABLE public.profile_textures (
    id uuid NOT NULL,
    created_at timestamp(6) with time zone NOT NULL,
    profile_id uuid NOT NULL,
    skin_model bigint NOT NULL,
    skin_file uuid,
    cape_file uuid
);

ALTER TABLE ONLY public.profile_textures
    ADD CONSTRAINT profile_textures_pkey PRIMARY KEY (id);

-- Token
   
CREATE TABLE public.tokens (
    access_token uuid NOT NULL,
    user_id uuid NOT NULL,
    client_token text NOT NULL,
    created_at timestamp(6) with time zone NOT NULL,
    profile_id uuid
);

ALTER TABLE ONLY public.tokens
    ADD CONSTRAINT tokens_pkey PRIMARY KEY (access_token);

-- UserInstance

CREATE TABLE public.user_instances (
    id uuid NOT NULL,
    user_id uuid NOT NULL,
    instance_id uuid NOT NULL
);

ALTER TABLE ONLY public.user_instances
    ADD CONSTRAINT user_instances_pkey PRIMARY KEY (id);

-- User

CREATE TABLE public.users (
    id uuid NOT NULL,
    nickname text NOT NULL,
    email text NOT NULL,
    password text NOT NULL,
    preferred_language text NOT NULL,
    permission bigint NOT NULL
);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

-- Indexes

CREATE INDEX index_game_profiles_by_name ON public.game_profiles USING btree (name);

CREATE INDEX index_game_profiles_by_owner_id ON public.game_profiles USING btree (owner_id);

CREATE INDEX index_profile_textures_by_profile_id ON public.profile_textures USING btree (profile_id);

CREATE INDEX index_tokens_by_profile_id ON public.tokens USING btree (profile_id);

CREATE INDEX index_tokens_by_user_id ON public.tokens USING btree (user_id);

CREATE INDEX index_user_instances_by_instance_id ON public.user_instances USING btree (instance_id);

CREATE INDEX index_user_instances_by_user_id ON public.user_instances USING btree (user_id);

CREATE UNIQUE INDEX index_users_by_email ON public.users USING btree (email);
