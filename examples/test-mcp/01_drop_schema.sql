-- =============================================================================
-- test-mcp — Schema DDL
-- Test database for the firebird-5-mcp MCP server.
--
-- This script is idempotent: it can be run multiple times.
-- Existing objects are dropped before being recreated.
--
-- Usage:
--   1. Create the database:  firebird_create_database  (via MCP)
--   2. Run this script:      firebird_run_script  (via MCP)
--   3. Run seed.sql:         firebird_run_script  (via MCP)
-- =============================================================================

-- ---------------------------------------------------------------------------
-- DROP (order respects dependencies: dependents before referenced)
-- Firebird does not support IF EXISTS for DROP, so we use EXECUTE BLOCK
-- to check existence before dropping.
-- ---------------------------------------------------------------------------

-- Altera o terminador padrão de ";" para "^"
SET TERM ^ ;

-- Triggers
EXECUTE BLOCK AS
DECLARE VARIABLE t_name VARCHAR(63);
BEGIN
    FOR SELECT rdb$trigger_name
        FROM rdb$triggers
        WHERE trim(rdb$trigger_name) in ('TG_USER_EVENT', 'TG_USER_UPDATE_EVENT', 'TG_USER_DELETE_EVENT', 'TG_APPROVAL_BEFORE_INSERT')
          AND COALESCE(rdb$system_flag, 0) = 0
        INTO :t_name
    DO
    BEGIN
        EXECUTE STATEMENT 'DROP TRIGGER ' || TRIM(:t_name);
    END
END^

-- Procedures
EXECUTE BLOCK AS
DECLARE VARIABLE p_name VARCHAR(63);
BEGIN
    FOR SELECT rdb$procedure_name
        FROM rdb$procedures
        WHERE trim(rdb$procedure_name) in ('SP_LIST_USERS', 'SP_COUNT_APPROVALS')
          AND COALESCE(rdb$system_flag, 0) = 0
        INTO :p_name
    DO
    BEGIN
        EXECUTE STATEMENT 'DROP PROCEDURE ' || TRIM(:p_name);
    END
END^

-- Functions
EXECUTE BLOCK AS
DECLARE VARIABLE f_name VARCHAR(63);
BEGIN
    FOR SELECT rdb$function_name
        FROM rdb$functions
        WHERE trim(rdb$function_name) in ('FC_CALCULATE_AGE')
          AND COALESCE(rdb$system_flag, 0) = 0
        INTO :f_name
    DO
    BEGIN
        EXECUTE STATEMENT 'DROP FUNCTION ' || TRIM(:f_name);
    END
END^

-- Sequences
EXECUTE BLOCK AS
DECLARE VARIABLE s_name VARCHAR(63);
BEGIN
    FOR SELECT rdb$generator_name
        FROM rdb$generators
        WHERE trim(rdb$generator_name) in ('GEN_USER_ID', 'GEN_APPROVAL_ID')
          AND COALESCE(rdb$system_flag, 0) = 0
        INTO :s_name
    DO
    BEGIN
        EXECUTE STATEMENT 'DROP SEQUENCE ' || TRIM(:s_name);
    END
END^

-- Tables (dependents first)
EXECUTE BLOCK AS
DECLARE VARIABLE t_name VARCHAR(63);
BEGIN
    FOR SELECT rdb$relation_name
        FROM rdb$relations
        WHERE trim(rdb$relation_name) in ('ACTION_LOG', 'APPROVAL', 'USER_PROFILE', 'PROFILE_PERMISSION', 'PROFILE_SCREEN', 'USER_DEPARTMENT', 'USERS', 'DEPARTMENT', 'SCREEN', 'PERMISSION', 'PROFILE', 'BRANCH')
          AND COALESCE(rdb$system_flag, 0) = 0
          AND rdb$view_blr IS NULL
        ORDER BY CASE trim(rdb$relation_name)
            WHEN 'ACTION_LOG' THEN 1
            WHEN 'APPROVAL' THEN 2
            WHEN 'USER_PROFILE' THEN 3
            WHEN 'PROFILE_PERMISSION' THEN 4
            WHEN 'PROFILE_SCREEN' THEN 5
            WHEN 'USER_DEPARTMENT' THEN 6
            WHEN 'USERS' THEN 7
            WHEN 'DEPARTMENT' THEN 8
            WHEN 'SCREEN' THEN 9
            WHEN 'PERMISSION' THEN 10
            WHEN 'PROFILE' THEN 11
            WHEN 'BRANCH' THEN 12
        END
        INTO :t_name
    DO
    BEGIN
        EXECUTE STATEMENT 'DROP TABLE ' || TRIM(:t_name);
    END
END^

-- Views
EXECUTE BLOCK AS
DECLARE VARIABLE v_name VARCHAR(63);
BEGIN
    FOR SELECT rdb$relation_name
        FROM rdb$relations
        WHERE trim(rdb$relation_name) in ('VW_USER_DEPARTMENT', 'VW_APPROVAL_SUMMARY')
          AND COALESCE(rdb$system_flag, 0) = 0
          AND rdb$view_blr IS NOT NULL
        INTO :v_name
    DO
    BEGIN
        EXECUTE STATEMENT 'DROP VIEW ' || TRIM(:v_name);
    END
END^

-- Domains
EXECUTE BLOCK AS
DECLARE VARIABLE d_name VARCHAR(63);
BEGIN
    FOR SELECT rdb$field_name
        FROM rdb$fields
        WHERE trim(rdb$field_name) in ('D_BOOLEAN', 'D_INTEGER', 'D_TIMESTAMP', 'D_VARCHAR_100', 'D_VARCHAR_200')
          AND COALESCE(rdb$system_flag, 0) = 0
        INTO :d_name
    DO
    BEGIN
        EXECUTE STATEMENT 'DROP DOMAIN ' || TRIM(:d_name);
    END
END^

-- Restaura o terminador padrão de volta para ";"
SET TERM ; ^
