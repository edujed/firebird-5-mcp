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

-- ---------------------------------------------------------------------------
-- DOMAINS
-- ---------------------------------------------------------------------------


-- ---------------------------------------------------------------------------
-- TRIGGERS
-- ---------------------------------------------------------------------------

-- Altera o terminador padrão de ";" para "^"
SET TERM ^ ;

CREATE TRIGGER TG_USER_EVENT FOR USERS
AFTER INSERT OR UPDATE OR DELETE
AS
BEGIN
    POST_EVENT 'user_changed';
END
^

CREATE TRIGGER TG_USER_UPDATE_EVENT FOR USERS
AFTER UPDATE
AS
BEGIN
    POST_EVENT 'user_updated';
END
^

CREATE TRIGGER TG_USER_DELETE_EVENT FOR USERS
AFTER DELETE
AS
BEGIN
    POST_EVENT 'user_deleted';
END
^

CREATE TRIGGER TG_APPROVAL_BEFORE_INSERT FOR APPROVAL
BEFORE INSERT
AS
BEGIN
    IF (NEW.STATUS IS NULL) THEN
        NEW.STATUS = 'PENDING';
    IF (NEW.CREATED_AT IS NULL) THEN
        NEW.CREATED_AT = CURRENT_TIMESTAMP;
END
^

-- ---------------------------------------------------------------------------
-- FUNCTIONS
-- ---------------------------------------------------------------------------

CREATE FUNCTION FC_CALCULATE_AGE(
    BIRTH_DATE DATE
) RETURNS INTEGER
AS
BEGIN
    IF (BIRTH_DATE IS NULL) THEN
        RETURN NULL;
    RETURN DATEADD(YEAR, -DATEDIFF(YEAR, BIRTH_DATE, CURRENT_DATE), BIRTH_DATE);
END
^

-- ---------------------------------------------------------------------------
-- PROCEDURES
-- ---------------------------------------------------------------------------

CREATE PROCEDURE SP_LIST_USERS(
    P_BRANCH_ID INTEGER
) RETURNS (
    ID          INTEGER,
    NAME        VARCHAR(100),
    EMAIL       VARCHAR(200),
    DEPT_NAME   VARCHAR(100),
    BRANCH_CODE VARCHAR(10),
    ACTIVE      SMALLINT
)
AS
BEGIN
    FOR
        SELECT
            U.ID,
            U.NAME,
            U.EMAIL,
            D.NAME,
            B.CODE,
            U.ACTIVE
        FROM USERS U
        JOIN DEPARTMENT D ON D.ID = U.DEPARTMENT_ID
        JOIN BRANCH B ON B.ID = D.BRANCH_ID
        WHERE (:P_BRANCH_ID IS NULL OR D.BRANCH_ID = :P_BRANCH_ID)
        ORDER BY U.NAME
    INTO :ID, :NAME, :EMAIL, :DEPT_NAME, :BRANCH_CODE, :ACTIVE
    DO
    BEGIN
        SUSPEND;
    END
END
^

CREATE PROCEDURE SP_COUNT_APPROVALS(
    P_USER_ID INTEGER,
    P_STATUS  VARCHAR(20)
) RETURNS (
    CNT INTEGER
)
AS
BEGIN
    CNT = 0;
    SELECT COUNT(*)
    FROM APPROVAL
    WHERE (:P_USER_ID IS NULL OR REQUESTER_ID = :P_USER_ID)
      AND (:P_STATUS IS NULL OR STATUS = :P_STATUS)
    INTO :CNT;
    SUSPEND;
END
^

-- Restaura o terminador padrão de volta para ";"
SET TERM ; ^
