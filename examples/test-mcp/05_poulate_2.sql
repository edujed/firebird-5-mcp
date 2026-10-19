-- =============================================================================
-- test-mcp — Seed Data
-- Populates the test database with realistic sample data.
--
-- This script is idempotent: it deletes existing data before inserting.
-- Run AFTER 02_create_schema.sql and 03_create_triggers.sql.
--
-- Volumes (approximate):
--   BRANCH:              4
--   DEPARTMENT:         35 (15 physical + 20 temporary)
--   USERS:             100
--   PROFILE:             5
--   PERMISSION:         10
--   SCREEN:              8
--   USER_DEPARTMENT:  ~500 (100 primary + 1-5 temporary per user)
--   USER_PROFILE:      100
--   PROFILE_PERMISSION: 25
--   PROFILE_SCREEN:     20
--   APPROVAL:         ~350 (2-5 physical + 0-2 temporary per user)
--   ACTION_LOG:       ~1500
-- =============================================================================

-- Altera o terminador padrão de ";" para "^"
SET TERM ^ ;

-- ---------------------------------------------------------------------------
-- USERS (100 rows) + USER_DEPARTMENT (~500 rows)
-- Each user: 1 primary physical dept + 1-5 temporary depts
-- ---------------------------------------------------------------------------

EXECUTE BLOCK AS
DECLARE VARIABLE i INTEGER DEFAULT 1;
DECLARE VARIABLE first_name VARCHAR(50);
DECLARE VARIABLE last_name VARCHAR(50);
DECLARE VARIABLE email VARCHAR(200);
DECLARE VARIABLE salary NUMERIC(14,2);
DECLARE VARIABLE hired_at DATE;
DECLARE VARIABLE primary_dept INTEGER;
DECLARE VARIABLE temp_count INTEGER;
DECLARE VARIABLE j INTEGER;
DECLARE VARIABLE temp_dept INTEGER;
DECLARE VARIABLE is_active BOOLEAN;
DECLARE VARIABLE cnt INTEGER;
DECLARE VARIABLE next_user_dep INTEGER;

BEGIN

    SELECT FIRST 1 COALESCE(MAX(id), 0) + 1
      FROM USER_DEPARTMENT
      INTO :next_user_dep;

    WHILE (i <= 100) DO
    BEGIN
        first_name = FC_FIRST_NAME(i);
        last_name = FC_LAST_NAME(i);
        email = LOWER(first_name) || '.' || LOWER(last_name) || '-' || :i || '@company.com';

        salary = 40000 + FLOOR(RAND() * 80000);
        hired_at = DATEADD(DAY, FLOOR(RAND() * 5000), DATE '2018-01-01');
        primary_dept = 1 + FLOOR(RAND() * 15);
        is_active = RAND() > 0.1;

        INSERT INTO USERS (ID, DEPARTMENT_ID, NAME, EMAIL, PASSWORD_HASH, ACTIVE, SALARY, HIRED_AT, CREATED_AT, UPDATED_AT)
        VALUES (:i, :primary_dept, :first_name || ' ' || :last_name, :email, 'hash_' || CAST(:i AS VARCHAR(10)), :is_active, :salary, :hired_at, :hired_at, CURRENT_TIMESTAMP);

        INSERT INTO USER_DEPARTMENT (ID, USER_ID, DEPARTMENT_ID, PRIMARY_FLAG, START_DATE, END_DATE, ACTIVE)
        VALUES (:next_user_dep, :i, :primary_dept, TRUE, :hired_at, NULL, :is_active);
        next_user_dep = next_user_dep +1;

        temp_count = 1 + FLOOR(RAND() * 5);
        j = 1;
        WHILE (j <= temp_count) DO
        BEGIN
            temp_dept = 16 + FLOOR(RAND() * 20);
            -- Check if already assigned (avoid duplicates)
            SELECT FIRST 1 COUNT(*)
              FROM USER_DEPARTMENT
              WHERE USER_ID = :i AND DEPARTMENT_ID = :temp_dept
              INTO :cnt;
            IF (cnt = 0) THEN
            BEGIN
                INSERT INTO USER_DEPARTMENT (ID, USER_ID, DEPARTMENT_ID, PRIMARY_FLAG, START_DATE, END_DATE, ACTIVE)
                VALUES (:next_user_dep, :i, :temp_dept, FALSE,
                        DATEADD(MONTH, :j, :hired_at),
                        DATEADD(MONTH, :j + 6, :hired_at),
                        TRUE);
                next_user_dep = next_user_dep +1;
            END
            j = j + 1;
        END

        i = i + 1;
    END
END^

-- ---------------------------------------------------------------------------
-- USER_PROFILE (100 rows)
-- ---------------------------------------------------------------------------

EXECUTE BLOCK AS
DECLARE VARIABLE i INTEGER DEFAULT 1;
DECLARE VARIABLE profile_id INTEGER;
BEGIN
    WHILE (i <= 100) DO
    BEGIN
        profile_id = 1 + FLOOR(RAND() * 5);

        INSERT INTO USER_PROFILE (ID, USER_ID, PROFILE_ID)
        VALUES (NEXT VALUE FOR GEN_USER_ID, :i, :profile_id);

        i = i + 1;
    END
END^

-- ---------------------------------------------------------------------------
-- APPROVAL (~350 rows: 2-5 physical + 0-2 temporary per user)
-- ---------------------------------------------------------------------------

EXECUTE BLOCK AS
DECLARE VARIABLE i INTEGER DEFAULT 1;
DECLARE VARIABLE approver_id INTEGER;
DECLARE VARIABLE status VARCHAR(20);
DECLARE VARIABLE reason VARCHAR(500);
DECLARE VARIABLE approved_at TIMESTAMP;
DECLARE VARIABLE created_at TIMESTAMP;
DECLARE VARIABLE phys_count INTEGER;
DECLARE VARIABLE temp_count INTEGER;
DECLARE VARIABLE j INTEGER;
BEGIN
    WHILE (i <= 100) DO
    BEGIN
        -- Physical approvals: 2-5
        phys_count = 2 + FLOOR(RAND() * 4);
        j = 1;
        WHILE (j <= phys_count) DO
        BEGIN
            approver_id = 1 + FLOOR(RAND() * 100);
            IF (approver_id <> i) THEN
            BEGIN
                status = CASE FLOOR(RAND() * 3)
                    WHEN 0 THEN 'APPROVED'
                    WHEN 1 THEN 'PENDING'
                    ELSE 'REJECTED'
                END;
                reason = 'Request ' || CAST(j AS VARCHAR(10)) || ' from user ' || CAST(i AS VARCHAR(10));
                created_at = DATEADD(DAY, FLOOR(RAND() * 365), DATE '2024-01-01');
                approved_at = CASE WHEN status = 'APPROVED' THEN DATEADD(DAY, 1, created_at) ELSE NULL END;

                INSERT INTO APPROVAL (ID, REQUESTER_ID, APPROVER_ID, STATUS, REASON, APPROVED_AT, CREATED_AT)
                VALUES (NEXT VALUE FOR GEN_APPROVAL_ID, :i, :approver_id, :status, :reason, :approved_at, :created_at);
            END
            j = j + 1;
        END

        -- Temporary approvals: 0-2
        temp_count = FLOOR(RAND() * 3);
        j = 1;
        WHILE (j <= temp_count) DO
        BEGIN
            approver_id = 1 + FLOOR(RAND() * 100);
            IF (approver_id <> i) THEN
            BEGIN
                status = CASE FLOOR(RAND() * 4)
                    WHEN 0 THEN 'APPROVED'
                    WHEN 1 THEN 'PENDING'
                    WHEN 2 THEN 'REJECTED'
                    ELSE 'APPROVED'
                END;
                reason = 'Temporary request ' || CAST(j AS VARCHAR(10)) || ' from user ' || CAST(i AS VARCHAR(10));
                created_at = DATEADD(DAY, FLOOR(RAND() * 365), DATE '2024-01-01');
                approved_at = CASE WHEN status = 'APPROVED' THEN DATEADD(DAY, 1, created_at) ELSE NULL END;

                INSERT INTO APPROVAL (ID, REQUESTER_ID, APPROVER_ID, STATUS, REASON, APPROVED_AT, CREATED_AT)
                VALUES (NEXT VALUE FOR GEN_APPROVAL_ID, :i, :approver_id, :status, :reason, :approved_at, :created_at);
            END
            j = j + 1;
        END

        i = i + 1;
    END
END^

-- ---------------------------------------------------------------------------
-- ACTION_LOG (~1500 rows)
-- ---------------------------------------------------------------------------

EXECUTE BLOCK AS
DECLARE VARIABLE i INTEGER DEFAULT 1;
DECLARE VARIABLE user_id INTEGER;
DECLARE VARIABLE action VARCHAR(50);
DECLARE VARIABLE entity VARCHAR(50);
DECLARE VARIABLE entity_id INTEGER;
DECLARE VARIABLE details VARCHAR(1000);
DECLARE VARIABLE created_at TIMESTAMP;
BEGIN
    WHILE (i <= 1500) DO
    BEGIN
        user_id = 1 + FLOOR(RAND() * 100);
        action = CASE FLOOR(RAND() * 4)
            WHEN 0 THEN 'CREATE'
            WHEN 1 THEN 'UPDATE'
            WHEN 2 THEN 'DELETE'
            ELSE 'VIEW'
        END;
        entity = CASE FLOOR(RAND() * 5)
            WHEN 0 THEN 'USER'
            WHEN 1 THEN 'APPROVAL'
            WHEN 2 THEN 'DEPARTMENT'
            WHEN 3 THEN 'PROFILE'
            ELSE 'SCREEN'
        END;
        entity_id = 1 + FLOOR(RAND() * 100);
        details = action || ' ' || entity || ' #' || CAST(entity_id AS VARCHAR(10)) || ' by user ' || CAST(user_id AS VARCHAR(10));
        created_at = DATEADD(HOUR, FLOOR(RAND() * 8760), TIMESTAMP '2024-01-01 00:00:00');

        INSERT INTO ACTION_LOG (ID, USER_ID, ACTION, ENTITY, ENTITY_ID, DETAILS, CREATED_AT)
        VALUES (:i, :user_id, :action, :entity, :entity_id, :details, :created_at);

        i = i + 1;
    END
END^

-- Restaura o terminador padrão de volta para ";"
SET TERM ; ^
