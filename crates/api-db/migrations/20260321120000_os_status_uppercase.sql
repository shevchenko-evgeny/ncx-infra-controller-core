-- Normalize operating_systems.status to uppercase to match the TenantState proto enum.
UPDATE operating_systems SET status = UPPER(status) WHERE status != UPPER(status);
ALTER TABLE operating_systems ALTER COLUMN status SET DEFAULT 'PROVISIONING';
