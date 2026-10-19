-- =============================================================================
-- test-mcp — Additional Views
-- Creates views with JOINs for export/import testing.
--
-- Run AFTER 02_create_schema.sql (which creates the base views).
-- This script is idempotent: it drops and recreates the views.
-- =============================================================================

-- ---------------------------------------------------------------------------
-- DROP existing views (idempotent)
-- ---------------------------------------------------------------------------

-- Views are dropped by 01_drop_schema.sql before this script runs.
-- If a view already exists, the CREATE VIEW will fail (use 01_drop_schema.sql first).

-- ---------------------------------------------------------------------------
-- VW_EMPLOYEES
-- JOIN USERS + DEPARTMENT (primary) + BRANCH + PROFILE
-- ~100 rows
-- ---------------------------------------------------------------------------

CREATE VIEW VW_EMPLOYEES AS
SELECT
    U.ID          AS USER_ID,
    U.NAME        AS USER_NAME,
    U.EMAIL       AS USER_EMAIL,
    U.ACTIVE      AS USER_ACTIVE,
    U.SALARY      AS SALARY,
    U.HIRED_AT    AS HIRED_AT,
    D.ID          AS DEPARTMENT_ID,
    D.NAME        AS DEPARTMENT_NAME,
    D.CODE        AS DEPARTMENT_CODE,
    D.TYPE        AS DEPARTMENT_TYPE,
    B.ID          AS BRANCH_ID,
    B.NAME        AS BRANCH_NAME,
    B.CODE        AS BRANCH_CODE,
    P.ID          AS PROFILE_ID,
    P.NAME        AS PROFILE_NAME
FROM USERS U
JOIN DEPARTMENT D ON D.ID = U.DEPARTMENT_ID
JOIN BRANCH B ON B.ID = D.BRANCH_ID
LEFT JOIN USER_PROFILE UP ON UP.USER_ID = U.ID
LEFT JOIN PROFILE P ON P.ID = UP.PROFILE_ID;

-- ---------------------------------------------------------------------------
-- VW_USER_ASSIGNMENTS
-- JOIN USERS + USER_DEPARTMENT + DEPARTMENT + BRANCH
-- ~500 rows (100 primary + 1-5 temporary per user)
-- ---------------------------------------------------------------------------

CREATE VIEW VW_USER_ASSIGNMENTS AS
SELECT
    U.ID          AS USER_ID,
    U.NAME        AS USER_NAME,
    U.EMAIL       AS USER_EMAIL,
    UD.ID         AS ASSIGNMENT_ID,
    UD.PRIMARY_FLAG,
    UD.START_DATE,
    UD.END_DATE,
    UD.ACTIVE     AS ASSIGNMENT_ACTIVE,
    D.ID          AS DEPARTMENT_ID,
    D.NAME        AS DEPARTMENT_NAME,
    D.CODE        AS DEPARTMENT_CODE,
    D.TYPE        AS DEPARTMENT_TYPE,
    B.ID          AS BRANCH_ID,
    B.NAME        AS BRANCH_NAME,
    B.CODE        AS BRANCH_CODE
FROM USERS U
JOIN USER_DEPARTMENT UD ON UD.USER_ID = U.ID
JOIN DEPARTMENT D ON D.ID = UD.DEPARTMENT_ID
JOIN BRANCH B ON B.ID = D.BRANCH_ID;

-- ---------------------------------------------------------------------------
-- VW_APPROVAL_FLOW
-- JOIN APPROVAL + USERS (requester) + USERS (approver) + DEPARTMENT
-- ~350 rows
-- ---------------------------------------------------------------------------

CREATE VIEW VW_APPROVAL_FLOW AS
SELECT
    A.ID              AS APPROVAL_ID,
    A.STATUS          AS STATUS,
    A.REASON          AS REASON,
    A.CREATED_AT      AS CREATED_AT,
    A.APPROVED_AT     AS APPROVED_AT,
    R.ID              AS REQUESTER_ID,
    R.NAME            AS REQUESTER_NAME,
    R.EMAIL           AS REQUESTER_EMAIL,
    RD.ID             AS REQUESTER_DEPT_ID,
    RD.NAME           AS REQUESTER_DEPT_NAME,
    P.ID              AS APPROVER_ID,
    P.NAME            AS APPROVER_NAME,
    P.EMAIL           AS APPROVER_EMAIL,
    PD.ID             AS APPROVER_DEPT_ID,
    PD.NAME           AS APPROVER_DEPT_NAME
FROM APPROVAL A
JOIN USERS R ON R.ID = A.REQUESTER_ID
JOIN USERS P ON P.ID = A.APPROVER_ID
LEFT JOIN DEPARTMENT RD ON RD.ID = R.DEPARTMENT_ID
LEFT JOIN DEPARTMENT PD ON PD.ID = P.DEPARTMENT_ID;

-- ---------------------------------------------------------------------------
-- VW_DEPT_HEADCOUNT
-- Aggregated by department: COUNT, AVG salary
-- ~35 rows
-- ---------------------------------------------------------------------------

CREATE VIEW VW_DEPT_HEADCOUNT AS
SELECT
    D.ID          AS DEPARTMENT_ID,
    D.NAME        AS DEPARTMENT_NAME,
    D.CODE        AS DEPARTMENT_CODE,
    D.TYPE        AS DEPARTMENT_TYPE,
    B.ID          AS BRANCH_ID,
    B.NAME        AS BRANCH_NAME,
    COUNT(U.ID)   AS EMPLOYEE_COUNT,
    AVG(U.SALARY) AS AVG_SALARY,
    MIN(U.HIRED_AT) AS EARLIEST_HIRE,
    MAX(U.HIRED_AT) AS LATEST_HIRE
FROM DEPARTMENT D
JOIN BRANCH B ON B.ID = D.BRANCH_ID
LEFT JOIN USERS U ON U.DEPARTMENT_ID = D.ID
GROUP BY D.ID, D.NAME, D.CODE, D.TYPE, B.ID, B.NAME;
