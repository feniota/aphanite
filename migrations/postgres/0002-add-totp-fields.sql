ALTER TABLE public.users ADD COLUMN totp_secret text;
ALTER TABLE public.users ADD COLUMN totp_active boolean NOT NULL DEFAULT false;
