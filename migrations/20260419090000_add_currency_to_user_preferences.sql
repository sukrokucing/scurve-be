ALTER TABLE user_preferences ADD COLUMN currency TEXT NOT NULL DEFAULT 'IDR';
UPDATE user_preferences SET currency = 'IDR' WHERE currency IS NULL OR currency = '';
