-- Instance may refer to an operating system (design 0076). When set, overrides (os_user_data, os_ipxe_script, etc.) apply on top of the OS. When NULL, OS is derived from instance columns only.

ALTER TABLE instances
    ADD COLUMN IF NOT EXISTS operating_system_id uuid REFERENCES operating_systems(id);

CREATE INDEX IF NOT EXISTS instances_operating_system_id_idx ON instances(operating_system_id);
